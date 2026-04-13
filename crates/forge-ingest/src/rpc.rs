use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Raw transaction data from the RPC — meta kept as raw JSON
/// so the parser can access original camelCase field names directly
#[derive(Debug, Clone, Deserialize)]
pub struct RawTransaction {
    pub signature: String,
    pub slot: Option<u64>,
    #[serde(rename = "blockTime")]
    pub block_time: Option<i64>,
    pub meta: Option<serde_json::Value>,
    pub transaction: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionMeta {
    pub err: Option<serde_json::Value>,
    pub fee: Option<u64>,
    #[serde(default)]
    pub pre_balances: Vec<u64>,
    #[serde(default)]
    pub post_balances: Vec<u64>,
    #[serde(default)]
    pub pre_token_balances: Vec<TokenBalance>,
    #[serde(default)]
    pub post_token_balances: Vec<TokenBalance>,
    #[serde(default)]
    pub inner_instructions: Vec<serde_json::Value>,
    #[serde(default)]
    pub log_messages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenBalance {
    pub account_index: Option<u8>,
    pub mint: Option<String>,
    pub owner: Option<String>,
    pub ui_token_amount: Option<UiTokenAmount>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiTokenAmount {
    pub amount: Option<String>,
    pub decimals: Option<u8>,
    pub ui_amount: Option<f64>,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<serde_json::Value>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

pub struct RpcClient {
    client: reqwest::Client,
    rpc_url: String,
}

impl RpcClient {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_url: rpc_url.to_string(),
        }
    }

    /// Fetch a full transaction by signature
    pub async fn get_transaction(&self, signature: &str) -> Result<Option<RawTransaction>> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getTransaction",
            "params": [
                signature,
                {
                    "encoding": "jsonParsed",
                    "maxSupportedTransactionVersion": 0
                }
            ]
        });

        let resp: RpcResponse = self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await?
            .json()
            .await
            .context("Failed to parse getTransaction response")?;

        if let Some(err) = resp.error {
            warn!(
                "RPC error for {}: {} - {}",
                signature, err.code, err.message
            );
            return Ok(None);
        }

        match resp.result {
            Some(serde_json::Value::Null) | None => Ok(None),
            Some(val) => {
                let mut raw: RawTransaction =
                    serde_json::from_value(val).context("Failed to deserialize transaction")?;
                raw.signature = signature.to_string();
                Ok(Some(raw))
            }
        }
    }

    /// Batch fetch transactions with rate limiting.
    /// Processes up to `max_per_second` requests per second to stay within API limits.
    pub async fn batch_fetch(
        &self,
        mut sig_rx: mpsc::Receiver<String>,
        tx_out: mpsc::Sender<RawTransaction>,
        max_per_second: u32,
    ) {
        let mut seen = HashSet::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            1000 / max_per_second as u64,
        ));

        let mut fetched: u64 = 0;

        while let Some(signature) = sig_rx.recv().await {
            // Dedup — same tx can appear from multiple program subscriptions
            if !seen.insert(signature.clone()) {
                continue;
            }

            // Keep seen set bounded
            if seen.len() > 10_000 {
                seen.clear();
            }

            // Rate limit
            interval.tick().await;

            let client = self.client.clone();
            let rpc_url = self.rpc_url.clone();
            let tx_out = tx_out.clone();

            tokio::spawn(async move {
                let rpc = RpcClient { client, rpc_url };

                match rpc.get_transaction(&signature).await {
                    Ok(Some(raw_tx)) => {
                        // Skip failed transactions
                        let has_error = raw_tx
                            .meta
                            .as_ref()
                            .and_then(|m| m.get("err"))
                            .is_some_and(|e| !e.is_null());
                        if has_error {
                            return;
                        }

                        if tx_out.send(raw_tx).await.is_err() {
                            warn!("Output channel closed");
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        debug!("Failed to fetch tx {}: {}", signature, e);
                    }
                }
            });

            fetched += 1;
            if fetched.is_multiple_of(50) {
                info!("Fetched {} transactions from RPC", fetched);
            }
        }
    }
}

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::{debug, warn};

/// Raw transaction data from the RPC
#[derive(Debug, Clone, Deserialize)]
pub struct RawTransaction {
    pub signature: String,
    pub slot: Option<u64>,
    #[serde(rename = "blockTime")]
    pub block_time: Option<i64>,
    pub meta: Option<TransactionMeta>,
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

    /// Batch fetch transactions from a channel of signatures.
    /// Collects signatures for `batch_window_ms`, then fetches them concurrently.
    pub async fn batch_fetch(
        &self,
        mut sig_rx: mpsc::Receiver<String>,
        tx_out: mpsc::Sender<RawTransaction>,
        max_concurrent: usize,
    ) {
        // Simple approach: fetch each signature as it arrives, with concurrency limit
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        while let Some(signature) = sig_rx.recv().await {
            let permit = semaphore.clone().acquire_owned().await;
            let client = self.client.clone();
            let rpc_url = self.rpc_url.clone();
            let tx_out = tx_out.clone();

            tokio::spawn(async move {
                let _permit = permit;
                let rpc = RpcClient { client, rpc_url };

                match rpc.get_transaction(&signature).await {
                    Ok(Some(raw_tx)) => {
                        // Skip failed transactions
                        if raw_tx.meta.as_ref().and_then(|m| m.err.as_ref()).is_some() {
                            debug!("Skipping failed tx: {}", signature);
                            return;
                        }

                        if tx_out.send(raw_tx).await.is_err() {
                            warn!("Output channel closed");
                        }
                    }
                    Ok(None) => {
                        debug!("Transaction not found: {}", signature);
                    }
                    Err(e) => {
                        warn!("Failed to fetch tx {}: {}", signature, e);
                    }
                }
            });
        }
    }
}

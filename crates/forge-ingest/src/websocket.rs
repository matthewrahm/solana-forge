use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

/// Connect to Solana WebSocket and subscribe to logs for the given program IDs.
/// Sends discovered transaction signatures to the channel.
pub async fn subscribe_logs(
    ws_url: &str,
    program_ids: &[&str],
    tx: mpsc::Sender<String>,
) -> Result<()> {
    info!("Connecting to WebSocket: {}", ws_url);

    let (ws_stream, _) = connect_async(ws_url)
        .await
        .context("Failed to connect to WebSocket")?;

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to logs for each program
    for (i, program_id) in program_ids.iter().enumerate() {
        let subscribe_msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": i + 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [program_id]
                },
                {
                    "commitment": "confirmed"
                }
            ]
        });

        write
            .send(Message::Text(subscribe_msg.to_string()))
            .await
            .context("Failed to send subscribe message")?;

        info!("Subscribed to logs for program: {}", program_id);
    }

    // Read incoming messages
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Some(signature) = extract_signature(&text) {
                    if tx.send(signature).await.is_err() {
                        warn!("Channel closed, stopping WebSocket listener");
                        break;
                    }
                }
            }
            Ok(Message::Ping(data)) => {
                let _ = write.send(Message::Pong(data)).await;
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket closed by server");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Extract the transaction signature from a logsSubscribe notification
fn extract_signature(text: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(text).ok()?;

    // logsSubscribe notifications have this structure:
    // { "method": "logsNotification", "params": { "result": { "value": { "signature": "..." } } } }
    json.get("params")?
        .get("result")?
        .get("value")?
        .get("signature")?
        .as_str()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_signature() {
        let msg = r#"{
            "jsonrpc": "2.0",
            "method": "logsNotification",
            "params": {
                "result": {
                    "context": { "slot": 123456 },
                    "value": {
                        "signature": "5VERv8NMHJEBNvkBZe8g6jNrmTJdCcEHPLHm3Fb2d9JgH",
                        "err": null,
                        "logs": ["Program log: Instruction: Swap"]
                    }
                },
                "subscription": 0
            }
        }"#;

        let sig = extract_signature(msg);
        assert_eq!(
            sig,
            Some("5VERv8NMHJEBNvkBZe8g6jNrmTJdCcEHPLHm3Fb2d9JgH".to_string())
        );
    }

    #[test]
    fn test_extract_signature_from_subscription_response() {
        // Subscription confirmation doesn't have a signature
        let msg = r#"{"jsonrpc":"2.0","result":12345,"id":1}"#;
        assert_eq!(extract_signature(msg), None);
    }
}

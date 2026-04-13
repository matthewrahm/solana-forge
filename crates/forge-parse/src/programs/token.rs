use crate::types::TransferEvent;
use chrono::DateTime;

/// Parse SPL Token transfers from the jsonParsed transaction format.
/// Solana RPC with `encoding: "jsonParsed"` already decodes Token Program instructions for us.
pub fn parse_transfers(
    signature: &str,
    slot: u64,
    block_time: i64,
    fee: u64,
    fee_payer: &str,
    tx_json: &serde_json::Value,
) -> Vec<TransferEvent> {
    let ts = match DateTime::from_timestamp(block_time, 0) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let instructions = match tx_json
        .get("message")
        .and_then(|m| m.get("instructions"))
        .and_then(|i| i.as_array())
    {
        Some(instrs) => instrs,
        None => return Vec::new(),
    };

    let mut events = Vec::new();

    for instr in instructions {
        let program_id = instr
            .get("programId")
            .or_else(|| instr.get("program"))
            .and_then(|p| p.as_str())
            .unwrap_or("");

        // Only parse Token Program instructions
        if program_id != crate::TOKEN_PROGRAM {
            continue;
        }

        let parsed = match instr.get("parsed") {
            Some(p) => p,
            None => continue,
        };

        let ix_type = parsed.get("type").and_then(|t| t.as_str()).unwrap_or("");

        let info = match parsed.get("info") {
            Some(i) => i,
            None => continue,
        };

        match ix_type {
            "transfer" | "transferChecked" => {
                let from = info
                    .get("source")
                    .or_else(|| info.get("authority"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let to = info
                    .get("destination")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let mint = info
                    .get("mint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let (amount, decimals) = if ix_type == "transferChecked" {
                    let amount_str = info
                        .get("tokenAmount")
                        .and_then(|t| t.get("amount"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("0");
                    let decimals = info
                        .get("tokenAmount")
                        .and_then(|t| t.get("decimals"))
                        .and_then(|d| d.as_u64())
                        .unwrap_or(0) as u8;
                    (amount_str.parse::<u64>().unwrap_or(0), decimals)
                } else {
                    let amount_str = info.get("amount").and_then(|a| a.as_str()).unwrap_or("0");
                    (amount_str.parse::<u64>().unwrap_or(0), 0)
                };

                if amount > 0 && !from.is_empty() && !to.is_empty() {
                    events.push(TransferEvent {
                        signature: signature.to_string(),
                        slot,
                        block_time: ts,
                        fee_lamports: fee,
                        fee_payer: fee_payer.to_string(),
                        mint,
                        from,
                        to,
                        amount,
                        decimals,
                    });
                }
            }
            _ => {}
        }
    }

    events
}

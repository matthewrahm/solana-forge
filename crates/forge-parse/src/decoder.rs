use crate::programs;
use crate::types::ParsedEvent;
use tracing::{debug, trace};

/// Decode a raw transaction JSON into parsed events.
/// Uses the balance-diff strategy for swaps and instruction parsing for transfers.
pub fn decode_transaction(
    signature: &str,
    slot: u64,
    block_time: i64,
    fee: u64,
    tx_json: &serde_json::Value,
    meta: &serde_json::Value,
) -> Vec<ParsedEvent> {
    let mut events = Vec::new();

    // Extract account keys
    let account_keys = extract_account_keys(tx_json);
    let fee_payer = account_keys.first().cloned().unwrap_or_default();

    // Check which programs were invoked
    let log_messages: Vec<String> = meta
        .get("logMessages")
        .and_then(|l| serde_json::from_value(l.clone()).ok())
        .unwrap_or_default();

    let has_raydium = log_messages
        .iter()
        .any(|l| l.contains(crate::RAYDIUM_AMM_V4));
    let has_jupiter = log_messages.iter().any(|l| l.contains(crate::JUPITER_V6));
    let has_pumpfun = log_messages.iter().any(|l| l.contains(crate::PUMPFUN));

    trace!(
        sig = &signature[..8],
        logs = log_messages.len(),
        has_raydium,
        has_jupiter,
        has_pumpfun,
        fee_payer = &fee_payer[..8.min(fee_payer.len())],
        "Decoding transaction"
    );

    // Parse swaps using balance-diff strategy
    if has_raydium || has_jupiter || has_pumpfun {
        if let Some(swap) =
            programs::balance_diff::parse_swap(signature, slot, block_time, fee, &fee_payer, meta)
        {
            events.push(ParsedEvent::Swap(swap));
        }
    }

    // Parse token transfers from parsed instructions
    let transfer_events =
        programs::token::parse_transfers(signature, slot, block_time, fee, &fee_payer, tx_json);
    for transfer in transfer_events {
        events.push(ParsedEvent::Transfer(transfer));
    }

    if events.is_empty() {
        debug!("No parseable events in tx: {}", signature);
    }

    events
}

fn extract_account_keys(tx_json: &serde_json::Value) -> Vec<String> {
    // jsonParsed format: transaction.message.accountKeys is an array of { pubkey, signer, ... }
    tx_json
        .get("message")
        .and_then(|m| m.get("accountKeys"))
        .and_then(|keys| keys.as_array())
        .map(|keys| {
            keys.iter()
                .filter_map(|k| {
                    // Could be a string or an object with "pubkey" field
                    k.as_str().map(|s| s.to_string()).or_else(|| {
                        k.get("pubkey")
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string())
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

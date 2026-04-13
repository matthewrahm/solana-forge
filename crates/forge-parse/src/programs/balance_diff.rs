use crate::types::{Platform, SwapEvent, TokenAmount};
use chrono::DateTime;
use tracing::debug;

/// Parse a swap using the balance-diff strategy.
/// Compares pre/post token balances to find what the signer sent and received.
/// This works for all DEXs (Raydium, Jupiter, PumpFun) without program-specific decoding.
pub fn parse_swap(
    signature: &str,
    slot: u64,
    block_time: i64,
    fee: u64,
    fee_payer: &str,
    meta: &serde_json::Value,
) -> Option<SwapEvent> {
    let pre_raw = meta.get("preTokenBalances")?;
    let post_raw = meta.get("postTokenBalances")?;
    let pre_balances = parse_token_balances(pre_raw);
    let post_balances = parse_token_balances(post_raw);

    // Find tokens where the fee_payer's balance changed
    let mut sent: Option<TokenAmount> = None;
    let mut received: Option<TokenAmount> = None;

    // Build a map of (owner, mint) -> (pre_amount, post_amount, decimals)
    let mut balance_changes: std::collections::HashMap<(String, String), (u64, u64, u8)> =
        std::collections::HashMap::new();

    for bal in &pre_balances {
        if bal.owner == fee_payer {
            balance_changes
                .entry((bal.owner.clone(), bal.mint.clone()))
                .or_insert((0, 0, bal.decimals))
                .0 = bal.amount;
        }
    }

    for bal in &post_balances {
        if bal.owner == fee_payer {
            balance_changes
                .entry((bal.owner.clone(), bal.mint.clone()))
                .or_insert((0, 0, bal.decimals))
                .1 = bal.amount;
        }
    }

    for ((_owner, mint), (pre, post, decimals)) in &balance_changes {
        if pre > post {
            // Token decreased — this is what was sent (swapped in)
            let diff = pre - post;
            if sent.is_none() || diff > sent.as_ref().unwrap().amount {
                sent = Some(TokenAmount {
                    mint: mint.clone(),
                    amount: diff,
                    decimals: *decimals,
                });
            }
        } else if post > pre {
            // Token increased — this is what was received (swapped out)
            let diff = post - pre;
            if received.is_none() || diff > received.as_ref().unwrap().amount {
                received = Some(TokenAmount {
                    mint: mint.clone(),
                    amount: diff,
                    decimals: *decimals,
                });
            }
        }
    }

    // Also check native SOL balance changes (pre/postBalances are lamport arrays)
    if let (Some(pre_sol), Some(post_sol)) = (
        meta.get("preBalances")
            .and_then(|b| b.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_u64()),
        meta.get("postBalances")
            .and_then(|b| b.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_u64()),
    ) {
        let fee_adjusted_pre = pre_sol.saturating_sub(fee);
        if fee_adjusted_pre > post_sol {
            let diff = fee_adjusted_pre - post_sol;
            if diff > 1_000_000 && (sent.is_none() || diff > sent.as_ref().unwrap().amount) {
                // Only count SOL if the diff is meaningful (> 0.001 SOL)
                sent = Some(TokenAmount {
                    mint: "So11111111111111111111111111111111111111112".to_string(),
                    amount: diff,
                    decimals: 9,
                });
            }
        } else if post_sol > fee_adjusted_pre {
            let diff = post_sol - fee_adjusted_pre;
            if diff > 1_000_000 && (received.is_none() || diff > received.as_ref().unwrap().amount)
            {
                received = Some(TokenAmount {
                    mint: "So11111111111111111111111111111111111111112".to_string(),
                    amount: diff,
                    decimals: 9,
                });
            }
        }
    }

    let (token_in, token_out) = match (sent, received) {
        (Some(s), Some(r)) => (s, r),
        _ => {
            debug!("Could not determine swap direction for tx: {}", signature);
            return None;
        }
    };

    // Determine platform from log messages
    let logs: Vec<String> = meta
        .get("logMessages")
        .and_then(|l| serde_json::from_value(l.clone()).ok())
        .unwrap_or_default();

    let platform = if logs.iter().any(|l| l.contains(crate::JUPITER_V6)) {
        Platform::Jupiter
    } else if logs.iter().any(|l| l.contains(crate::RAYDIUM_AMM_V4)) {
        Platform::Raydium
    } else if logs.iter().any(|l| l.contains(crate::PUMPFUN)) {
        Platform::PumpFun
    } else {
        Platform::Unknown
    };

    let ts = DateTime::from_timestamp(block_time, 0)?;

    Some(SwapEvent {
        signature: signature.to_string(),
        slot,
        block_time: ts,
        fee_lamports: fee,
        fee_payer: fee_payer.to_string(),
        platform,
        signer: fee_payer.to_string(),
        token_in,
        token_out,
        pool_address: None,
    })
}

struct BalanceEntry {
    owner: String,
    mint: String,
    amount: u64,
    decimals: u8,
}

fn parse_token_balances(balances: &serde_json::Value) -> Vec<BalanceEntry> {
    let arr = match balances.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|entry| {
            let owner = entry.get("owner")?.as_str()?.to_string();
            let mint = entry.get("mint")?.as_str()?.to_string();
            let ui_amount = entry.get("uiTokenAmount")?;
            let amount_str = ui_amount.get("amount")?.as_str()?;
            let amount: u64 = amount_str.parse().ok()?;
            let decimals = ui_amount.get("decimals")?.as_u64()? as u8;

            Some(BalanceEntry {
                owner,
                mint,
                amount,
                decimals,
            })
        })
        .collect()
}

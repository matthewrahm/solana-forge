use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct SwapRow {
    pub id: i64,
    pub signature: String,
    pub slot: i64,
    pub block_time: DateTime<Utc>,
    pub fee_lamports: i64,
    pub fee_payer: String,
    pub platform: String,
    pub signer: String,
    pub token_in_mint: String,
    pub token_in_amount: i64,
    pub token_in_decimals: i16,
    pub token_out_mint: String,
    pub token_out_amount: i64,
    pub token_out_decimals: i16,
    pub pool_address: Option<String>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TransferRow {
    pub id: i64,
    pub signature: String,
    pub slot: i64,
    pub block_time: DateTime<Utc>,
    pub fee_lamports: i64,
    pub fee_payer: String,
    pub mint: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: i64,
    pub decimals: i16,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TokenRow {
    pub mint: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub decimals: i16,
    pub first_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsOverview {
    pub total_swaps: i64,
    pub total_transfers: i64,
    pub unique_tokens: i64,
    pub unique_wallets: i64,
}

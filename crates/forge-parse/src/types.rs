use chrono::{DateTime, Utc};
use serde::Serialize;

/// A parsed blockchain event, ready for storage
#[derive(Debug, Clone, Serialize)]
pub enum ParsedEvent {
    Swap(SwapEvent),
    Transfer(TransferEvent),
}

#[derive(Debug, Clone, Serialize)]
pub struct SwapEvent {
    pub signature: String,
    pub slot: u64,
    pub block_time: DateTime<Utc>,
    pub fee_lamports: u64,
    pub fee_payer: String,
    pub platform: Platform,
    pub signer: String,
    pub token_in: TokenAmount,
    pub token_out: TokenAmount,
    pub pool_address: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransferEvent {
    pub signature: String,
    pub slot: u64,
    pub block_time: DateTime<Utc>,
    pub fee_lamports: u64,
    pub fee_payer: String,
    pub mint: String,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenAmount {
    pub mint: String,
    pub amount: u64,
    pub decimals: u8,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Raydium,
    Jupiter,
    PumpFun,
    PumpSwap,
    Unknown,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Raydium => "raydium",
            Platform::Jupiter => "jupiter",
            Platform::PumpFun => "pumpfun",
            Platform::PumpSwap => "pumpswap",
            Platform::Unknown => "unknown",
        }
    }
}

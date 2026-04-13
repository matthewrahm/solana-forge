pub mod rpc;
pub mod websocket;

/// Re-export program IDs from forge-parse
pub mod programs {
    pub use forge_parse::{JUPITER_V6, PUMPFUN, RAYDIUM_AMM_V4, TOKEN_PROGRAM};

    pub fn all() -> Vec<&'static str> {
        vec![TOKEN_PROGRAM, RAYDIUM_AMM_V4, JUPITER_V6, PUMPFUN]
    }
}

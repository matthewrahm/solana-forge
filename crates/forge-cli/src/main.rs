use anyhow::Result;
use clap::Parser;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};

use forge_ingest::rpc::{RawTransaction, RpcClient};
use forge_parse::decoder;
use forge_parse::types::ParsedEvent;

#[derive(Parser, Debug)]
#[command(
    name = "solana-forge",
    version,
    about = "Real-time Solana blockchain indexer"
)]
struct Args {
    /// Helius API key (or set HELIUS_API_KEY env var)
    #[arg(short = 'k', long)]
    api_key: Option<String>,

    /// Database URL (or set DATABASE_URL env var)
    #[arg(long)]
    database_url: Option<String>,

    /// API server port
    #[arg(short, long, default_value = "3001")]
    port: u16,

    /// Max RPC requests per second (stay within your Helius plan)
    #[arg(long, default_value = "5")]
    rpc_rate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "forge=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();

    let api_key = args
        .api_key
        .or_else(|| std::env::var("HELIUS_API_KEY").ok())
        .expect("Helius API key required: pass --api-key or set HELIUS_API_KEY");

    let database_url = args
        .database_url
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgres://localhost/solana_forge".to_string());

    // Connect to Postgres and run migrations
    info!("Connecting to database...");
    let pool = forge_store::create_pool(&database_url).await?;
    forge_store::run_migrations(&pool).await?;
    info!("Database ready");

    // Broadcast channel for real-time event streaming to WebSocket clients
    let (event_tx, _) = broadcast::channel::<String>(1000);

    // Channels for the pipeline
    let (sig_tx, sig_rx) = mpsc::channel::<String>(5000);
    let (raw_tx, mut raw_rx) = mpsc::channel::<RawTransaction>(1000);

    let rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={api_key}");
    let ws_url = format!("wss://mainnet.helius-rpc.com/?api-key={api_key}");

    // Start API server first so it's immediately available
    let api_pool = pool.clone();
    let api_port = args.port;
    let api_event_tx = event_tx.clone();
    tokio::spawn(async move {
        info!("Starting API server on http://localhost:{}", api_port);
        let app = forge_api::build_router(api_pool, api_event_tx);
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", api_port))
            .await
            .expect("Failed to bind API port");
        axum::serve(listener, app)
            .await
            .expect("API server crashed");
    });

    // Stage 1: WebSocket listener — discovers transaction signatures
    // Only subscribe to swap-producing programs (not Token Program — too much volume)
    let ws_url_clone = ws_url.clone();
    let sig_tx_clone = sig_tx.clone();
    tokio::spawn(async move {
        let programs = vec![
            forge_parse::RAYDIUM_AMM_V4,
            forge_parse::JUPITER_V6,
            forge_parse::PUMPFUN,
        ];
        loop {
            info!("Connecting to Solana WebSocket...");
            if let Err(e) = forge_ingest::websocket::subscribe_logs(
                &ws_url_clone,
                &programs,
                sig_tx_clone.clone(),
            )
            .await
            {
                error!("WebSocket disconnected: {}. Reconnecting in 5s...", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // Stage 2: RPC fetcher with rate limiting
    let rpc_rate = args.rpc_rate;
    let rpc_client = RpcClient::new(&rpc_url);
    tokio::spawn(async move {
        info!("RPC fetcher started (rate limit: {}/s)", rpc_rate);
        rpc_client.batch_fetch(sig_rx, raw_tx, rpc_rate).await;
    });

    // Stage 3: Parser + Store
    let store_pool = pool.clone();
    let mut event_count: u64 = 0;

    info!("Pipeline running. Waiting for transactions...");
    info!("API available at http://localhost:{}", args.port);
    info!("  GET /api/v1/swaps");
    info!("  GET /api/v1/transfers");
    info!("  GET /api/v1/stats");
    info!("  GET /api/v1/health");
    info!("  WS  /ws/events");

    while let Some(raw) = raw_rx.recv().await {
        let slot = raw.slot.unwrap_or(0);
        let block_time = raw.block_time.unwrap_or(0);
        let meta_json = raw.meta.unwrap_or(serde_json::Value::Null);
        let fee = meta_json.get("fee").and_then(|f| f.as_u64()).unwrap_or(0);

        let tx_json = raw.transaction.unwrap_or(serde_json::Value::Null);

        let events: Vec<ParsedEvent> = decoder::decode_transaction(
            &raw.signature,
            slot,
            block_time,
            fee,
            &tx_json,
            &meta_json,
        );

        if !events.is_empty() {
            event_count += events.len() as u64;

            for event in &events {
                match event {
                    ParsedEvent::Swap(swap) => {
                        info!(
                            "SWAP [{}] {} -> {} on {}",
                            &swap.signature[..8],
                            short_mint(&swap.token_in.mint),
                            short_mint(&swap.token_out.mint),
                            swap.platform.as_str()
                        );
                    }
                    ParsedEvent::Transfer(transfer) => {
                        info!(
                            "TRANSFER [{}] {} {} -> {}",
                            &transfer.signature[..8],
                            short_mint(&transfer.mint),
                            short_addr(&transfer.from),
                            short_addr(&transfer.to)
                        );
                    }
                }
            }

            // Broadcast events to WebSocket clients
            for event in &events {
                if let Ok(json) = serde_json::to_string(event) {
                    let _ = event_tx.send(json);
                }
            }

            if let Err(e) = forge_store::queries::insert_events(&store_pool, &events).await {
                error!("Failed to store: {}", e);
            }

            if event_count.is_multiple_of(25) {
                info!("--- Total indexed: {} events ---", event_count);
            }
        }
    }

    Ok(())
}

fn short_mint(mint: &str) -> String {
    if mint.len() > 8 {
        format!("{}..{}", &mint[..4], &mint[mint.len() - 4..])
    } else {
        mint.to_string()
    }
}

fn short_addr(addr: &str) -> String {
    if addr.len() > 8 {
        format!("{}..{}", &addr[..4], &addr[addr.len() - 4..])
    } else {
        addr.to_string()
    }
}

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
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
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "solana_forge=info,forge=info,tower_http=info".into()),
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

    // Channels for the pipeline: WebSocket -> RPC Fetcher -> Parser -> Store
    let (sig_tx, sig_rx) = mpsc::channel::<String>(5000);
    let (raw_tx, mut raw_rx) = mpsc::channel::<RawTransaction>(1000);

    let rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={api_key}");
    let ws_url = format!("wss://mainnet.helius-rpc.com/?api-key={api_key}");

    // Stage 1: WebSocket listener — discovers transaction signatures
    let ws_url_clone = ws_url.clone();
    let sig_tx_clone = sig_tx.clone();
    tokio::spawn(async move {
        let programs = forge_ingest::programs::all();
        let program_refs: Vec<&str> = programs.to_vec();
        loop {
            info!("Starting WebSocket listener...");
            if let Err(e) = forge_ingest::websocket::subscribe_logs(
                &ws_url_clone,
                &program_refs,
                sig_tx_clone.clone(),
            )
            .await
            {
                error!("WebSocket error: {}. Reconnecting in 5s...", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    });

    // Stage 2: RPC batch fetcher — fetches full transaction data
    let rpc_client = RpcClient::new(&rpc_url);
    tokio::spawn(async move {
        rpc_client.batch_fetch(sig_rx, raw_tx, 10).await;
    });

    // Stage 3: Parser + Store — decode and persist
    let store_pool = pool.clone();
    tokio::spawn(async move {
        let mut event_count: u64 = 0;

        while let Some(raw) = raw_rx.recv().await {
            let slot = raw.slot.unwrap_or(0);
            let block_time = raw.block_time.unwrap_or(0);
            let fee = raw.meta.as_ref().and_then(|m| m.fee).unwrap_or(0);

            let tx_json = raw.transaction.unwrap_or(serde_json::Value::Null);
            let meta_json = serde_json::to_value(&raw.meta).unwrap_or(serde_json::Value::Null);

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

                if let Err(e) = forge_store::queries::insert_events(&store_pool, &events).await {
                    error!("Failed to store events: {}", e);
                }

                if event_count.is_multiple_of(10) {
                    info!("Total events indexed: {}", event_count);
                }
            }
        }
    });

    // Stage 4: API server
    info!("Starting API server on port {}...", args.port);
    let app = forge_api::build_router(pool);
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    info!("API server listening on http://localhost:{}", args.port);
    axum::serve(listener, app).await?;

    Ok(())
}

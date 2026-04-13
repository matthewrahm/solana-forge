# solana-forge

[![CI](https://github.com/matthewrahm/solana-forge/actions/workflows/ci.yml/badge.svg)](https://github.com/matthewrahm/solana-forge/actions/workflows/ci.yml)
![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)

Real-time Solana blockchain indexer that decodes DEX swaps and token transfers, stores them in Postgres, and serves them via a REST API.

## How it works

```
Solana WebSocket (logsSubscribe)
        |
        |  transaction signatures
        v
   RPC Fetcher (rate-limited, deduped)
        |
        |  raw transaction JSON
        v
   Parser (balance-diff swap detection + SPL transfer decoding)
        |
        |  structured ParsedEvent (Swap | Transfer)
        v
   Postgres (swaps, transfers, tokens tables)
        |
        v
   REST API (axum)
```

Each stage runs as a separate tokio task connected by bounded mpsc channels. If Postgres falls behind, the channel fills, the parser pauses, and the RPC fetcher slows down. No data loss, no OOM.

## Supported programs

| Program | Detection |
|---------|-----------|
| **Raydium AMM V4** | Balance-diff on pre/post token balances |
| **Jupiter V6** | Balance-diff (works for any aggregator route) |
| **PumpFun** | Balance-diff on bonding curve operations |
| **SPL Token** | Parsed instruction decoding (transfer, transferChecked) |

The balance-diff strategy compares the fee payer's token balances before and after a transaction. The token that decreased is the input, the one that increased is the output. This works for any DEX without program-specific instruction decoding.

## API

All endpoints return JSON with pagination.

```
GET /api/v1/health

GET /api/v1/stats
    Returns: total_swaps, total_transfers, unique_tokens, unique_wallets

GET /api/v1/swaps
    ?wallet=<address>       Filter by signer
    ?platform=<name>        Filter by DEX (raydium, jupiter, pumpfun)
    ?token=<mint>           Filter by token (either side of swap)
    ?limit=50               Max results (capped at 100)
    ?offset=0               Pagination offset

GET /api/v1/transfers
    ?wallet=<address>       Filter by sender or recipient
    ?mint=<token_mint>      Filter by token
    ?limit=50
    ?offset=0
```

### Example response

```json
{
  "data": [
    {
      "signature": "5MZ63...",
      "slot": 412846024,
      "block_time": "2026-04-12T...",
      "platform": "pumpfun",
      "signer": "2cw7g...",
      "token_in_mint": "So111...1112",
      "token_in_amount": 100000000,
      "token_in_decimals": 9,
      "token_out_mint": "J7Jnh...pump",
      "token_out_amount": 5420000000,
      "token_out_decimals": 6
    }
  ],
  "pagination": { "limit": 50, "offset": 0, "count": 1 }
}
```

## Setup

### Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- PostgreSQL 14+
- A [Helius](https://helius.dev/) API key (free tier works)

### Install and run

```sh
git clone https://github.com/matthewrahm/solana-forge.git
cd solana-forge

# Create database
createdb solana_forge

# Configure
cp .env.example .env
# Edit .env: set HELIUS_API_KEY and DATABASE_URL

# Run (migrations execute automatically on startup)
cargo run -p forge-cli
```

The indexer starts immediately: WebSocket connects, transactions flow in, API serves on `http://localhost:3001`.

### Options

```
forge-cli [OPTIONS]

  -k, --api-key <KEY>           Helius API key (or HELIUS_API_KEY env)
      --database-url <URL>      Postgres URL (or DATABASE_URL env)
  -p, --port <PORT>             API port [default: 3001]
      --rpc-rate <N>            Max RPC requests/sec [default: 5]
```

## Project structure

```
solana-forge/
  Cargo.toml                    # Workspace root
  migrations/
    001_initial.sql             # Tables: swaps, transfers, tokens
    002_indexes.sql             # Query-path indexes
  crates/
    forge-ingest/               # WebSocket listener + RPC fetcher
      src/
        websocket.rs            # logsSubscribe for program activity
        rpc.rs                  # Rate-limited transaction fetcher with dedup
    forge-parse/                # Transaction decoder
      src/
        decoder.rs              # Dispatch by program ID
        types.rs                # ParsedEvent, SwapEvent, TransferEvent
        programs/
          balance_diff.rs       # Balance-diff swap detection (all DEXs)
          token.rs              # SPL Token transfer parsing
    forge-store/                # Postgres persistence
      src/
        models.rs               # sqlx row types
        queries.rs              # Insert + query with filters
    forge-api/                  # REST API
      src/
        routes/
          swaps.rs              # GET /api/v1/swaps
          transfers.rs          # GET /api/v1/transfers
          stats.rs              # GET /api/v1/stats + /health
    forge-cli/                  # Binary entry point
      src/
        main.rs                 # Wires all stages together
```

## Tech stack

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime, mpsc channels, rate limiting |
| `tokio-tungstenite` | WebSocket client for logsSubscribe |
| `reqwest` | RPC HTTP client |
| `serde` / `serde_json` | JSON parsing |
| `sqlx` | Async Postgres with query building |
| `axum` | REST API framework |
| `tower-http` | CORS + request tracing middleware |
| `tracing` | Structured logging |
| `clap` | CLI argument parsing |

## License

MIT

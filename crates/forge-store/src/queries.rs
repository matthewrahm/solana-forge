use anyhow::Result;
use sqlx::PgPool;

use crate::models::{StatsOverview, SwapRow, TransferRow};
use forge_parse::types::{ParsedEvent, SwapEvent, TransferEvent};

/// Insert a batch of parsed events into the database
pub async fn insert_events(pool: &PgPool, events: &[ParsedEvent]) -> Result<()> {
    for event in events {
        match event {
            ParsedEvent::Swap(swap) => insert_swap(pool, swap).await?,
            ParsedEvent::Transfer(transfer) => insert_transfer(pool, transfer).await?,
        }
    }
    Ok(())
}

async fn insert_swap(pool: &PgPool, swap: &SwapEvent) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO swaps (signature, slot, block_time, fee_lamports, fee_payer, platform, signer,
                          token_in_mint, token_in_amount, token_in_decimals,
                          token_out_mint, token_out_amount, token_out_decimals, pool_address)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ON CONFLICT (signature) DO NOTHING
        "#,
    )
    .bind(&swap.signature)
    .bind(swap.slot as i64)
    .bind(swap.block_time)
    .bind(swap.fee_lamports as i64)
    .bind(&swap.fee_payer)
    .bind(swap.platform.as_str())
    .bind(&swap.signer)
    .bind(&swap.token_in.mint)
    .bind(swap.token_in.amount as i64)
    .bind(swap.token_in.decimals as i16)
    .bind(&swap.token_out.mint)
    .bind(swap.token_out.amount as i64)
    .bind(swap.token_out.decimals as i16)
    .bind(&swap.pool_address)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_transfer(pool: &PgPool, transfer: &TransferEvent) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO transfers (signature, slot, block_time, fee_lamports, fee_payer,
                              mint, from_address, to_address, amount, decimals)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (signature, mint, from_address, to_address) DO NOTHING
        "#,
    )
    .bind(&transfer.signature)
    .bind(transfer.slot as i64)
    .bind(transfer.block_time)
    .bind(transfer.fee_lamports as i64)
    .bind(&transfer.fee_payer)
    .bind(&transfer.mint)
    .bind(&transfer.from)
    .bind(&transfer.to)
    .bind(transfer.amount as i64)
    .bind(transfer.decimals as i16)
    .execute(pool)
    .await?;

    Ok(())
}

/// Query swaps with optional filters
pub async fn get_swaps(
    pool: &PgPool,
    wallet: Option<&str>,
    platform: Option<&str>,
    token: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SwapRow>> {
    let rows = sqlx::query_as::<_, SwapRow>(
        r#"
        SELECT * FROM swaps
        WHERE ($1::text IS NULL OR signer = $1)
          AND ($2::text IS NULL OR platform = $2)
          AND ($3::text IS NULL OR token_in_mint = $3 OR token_out_mint = $3)
        ORDER BY block_time DESC
        LIMIT $4 OFFSET $5
        "#,
    )
    .bind(wallet)
    .bind(platform)
    .bind(token)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Query transfers with optional filters
pub async fn get_transfers(
    pool: &PgPool,
    wallet: Option<&str>,
    mint: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<TransferRow>> {
    let rows = sqlx::query_as::<_, TransferRow>(
        r#"
        SELECT * FROM transfers
        WHERE ($1::text IS NULL OR from_address = $1 OR to_address = $1)
          AND ($2::text IS NULL OR mint = $2)
        ORDER BY block_time DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(wallet)
    .bind(mint)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get overview stats
pub async fn get_stats(pool: &PgPool) -> Result<StatsOverview> {
    let swap_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM swaps")
        .fetch_one(pool)
        .await?;

    let transfer_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transfers")
        .fetch_one(pool)
        .await?;

    let unique_tokens: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT mint) FROM (
            SELECT token_in_mint AS mint FROM swaps
            UNION
            SELECT token_out_mint AS mint FROM swaps
            UNION
            SELECT mint FROM transfers
        ) t
        "#,
    )
    .fetch_one(pool)
    .await?;

    let unique_wallets: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(DISTINCT wallet) FROM (
            SELECT signer AS wallet FROM swaps
            UNION
            SELECT from_address AS wallet FROM transfers
            UNION
            SELECT to_address AS wallet FROM transfers
        ) t
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(StatsOverview {
        total_swaps: swap_count.0,
        total_transfers: transfer_count.0,
        unique_tokens: unique_tokens.0,
        unique_wallets: unique_wallets.0,
    })
}

pub mod models;
pub mod queries;

use anyhow::Result;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPool::connect(database_url).await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/001_initial.sql"))
        .execute(pool)
        .await?;
    sqlx::raw_sql(include_str!("../../../migrations/002_indexes.sql"))
        .execute(pool)
        .await?;
    Ok(())
}

mod stats;
mod swaps;
mod transfers;

use axum::Router;
use sqlx::PgPool;

pub fn api_routes() -> Router<PgPool> {
    Router::new()
        .merge(swaps::routes())
        .merge(transfers::routes())
        .merge(stats::routes())
}

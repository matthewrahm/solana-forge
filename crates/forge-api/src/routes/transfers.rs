use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;

use forge_store::queries;

#[derive(Deserialize)]
pub struct TransferQuery {
    wallet: Option<String>,
    mint: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

pub fn routes() -> Router<PgPool> {
    Router::new().route("/transfers", get(get_transfers))
}

async fn get_transfers(
    State(pool): State<PgPool>,
    Query(params): Query<TransferQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.min(100);

    match queries::get_transfers(
        &pool,
        params.wallet.as_deref(),
        params.mint.as_deref(),
        limit,
        params.offset,
    )
    .await
    {
        Ok(transfers) => Json(serde_json::json!({
            "data": transfers,
            "pagination": {
                "limit": limit,
                "offset": params.offset,
                "count": transfers.len()
            }
        })),
        Err(e) => Json(serde_json::json!({
            "error": e.to_string()
        })),
    }
}

use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use sqlx::PgPool;

use forge_store::queries;

#[derive(Deserialize)]
pub struct SwapQuery {
    wallet: Option<String>,
    platform: Option<String>,
    token: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

pub fn routes() -> Router<PgPool> {
    Router::new().route("/swaps", get(get_swaps))
}

async fn get_swaps(
    State(pool): State<PgPool>,
    Query(params): Query<SwapQuery>,
) -> Json<serde_json::Value> {
    let limit = params.limit.min(100);

    match queries::get_swaps(
        &pool,
        params.wallet.as_deref(),
        params.platform.as_deref(),
        params.token.as_deref(),
        limit,
        params.offset,
    )
    .await
    {
        Ok(swaps) => Json(serde_json::json!({
            "data": swaps,
            "pagination": {
                "limit": limit,
                "offset": params.offset,
                "count": swaps.len()
            }
        })),
        Err(e) => Json(serde_json::json!({
            "error": e.to_string()
        })),
    }
}

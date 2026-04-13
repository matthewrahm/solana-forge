use axum::{extract::State, routing::get, Json, Router};
use sqlx::PgPool;

use forge_store::queries;

pub fn routes() -> Router<PgPool> {
    Router::new()
        .route("/stats", get(get_stats))
        .route("/health", get(health))
}

async fn get_stats(State(pool): State<PgPool>) -> Json<serde_json::Value> {
    match queries::get_stats(&pool).await {
        Ok(stats) => Json(serde_json::json!(stats)),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

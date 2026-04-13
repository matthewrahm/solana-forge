pub mod routes;

use axum::{routing::get, Router};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(pool: PgPool, event_tx: broadcast::Sender<String>) -> Router {
    let api = Router::new()
        .nest("/api/v1", routes::api_routes())
        .with_state(pool);

    let ws = Router::new()
        .route("/ws/events", get(routes::ws::ws_handler))
        .with_state(event_tx);

    api.merge(ws)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

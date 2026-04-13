pub mod routes;

use axum::Router;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn build_router(pool: PgPool) -> Router {
    Router::new()
        .nest("/api/v1", routes::api_routes())
        .with_state(pool)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

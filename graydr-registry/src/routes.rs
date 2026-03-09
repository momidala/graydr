use std::sync::Arc;
use axum::{Router, routing::{get, put}};
use tower_http::trace::TraceLayer;
use tower_http::limit::RequestBodyLimitLayer;
use crate::{AppState, handlers};

pub fn build_router(state: Arc<AppState>) -> Router {
    // These URL patterns are derived from graydr/src/registry/client.rs — do NOT change them.
    // Any deviation produces silent 404 that looks like ModuleNotFound on the client.
    Router::new()
        .route("/api/v1/modules/{org}/{name}/{version}", put(handlers::publish::handle))
        .route("/api/v1/modules/{org}/{name}/{version}/content", get(handlers::content::handle))
        .route("/api/v1/modules/{org}/{name}/{version}/meta", get(handlers::meta::handle))
        .layer(RequestBodyLimitLayer::new(50 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

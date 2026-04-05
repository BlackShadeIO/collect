//! Axum application builder.

use std::time::Instant;

use axum::{Router, middleware, routing::get};
use tokio::sync::{broadcast, watch};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::orchestrator::scheduler::OrchestratorState;
use crate::types::{DepthState, IndicatorSnapshot, StorageRecord};

use super::auth::auth_middleware;
use super::routes;
use super::ws_stream;

/// Shared application state for all handlers.
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub started_at: Instant,
    pub broadcast_tx: broadcast::Sender<StorageRecord>,
    pub orch_state_rx: watch::Receiver<OrchestratorState>,
    pub indicator_rx: watch::Receiver<IndicatorSnapshot>,
    pub depth_state_rx: watch::Receiver<DepthState>,
}

pub fn build_app(state: AppState) -> Router {
    // Protected routes (require auth)
    let protected = Router::new()
        .route("/status", get(routes::status))
        .route("/stats", get(routes::stats))
        .route("/market", get(routes::list_markets))
        .route("/market/{epoch}", get(routes::get_market))
        .route("/download", get(routes::download))
        .route("/snapshot", get(routes::snapshot))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Public routes + WS
    Router::new()
        .route("/health", get(routes::health))
        .route("/ws", get(ws_stream::ws_handler))
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

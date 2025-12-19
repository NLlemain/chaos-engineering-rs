//! Chaos Web Dashboard
//!
//! A professional web UI for the chaos engineering framework.
//! Provides real-time monitoring, scenario management, and test execution.

mod api;
mod handlers;
pub mod load_test;
mod state;
mod templates;

pub use state::AppState;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// Configuration for the web server
#[derive(Clone, Debug)]
pub struct WebConfig {
    /// Port to listen on
    pub port: u16,
    /// Host to bind to
    pub host: String,
    /// Directory containing scenario YAML files
    pub scenarios_dir: PathBuf,
    /// Directory to store test results
    pub results_dir: PathBuf,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "127.0.0.1".to_string(),
            scenarios_dir: PathBuf::from("scenarios"),
            results_dir: PathBuf::from("test_results"),
        }
    }
}

/// Create the main application router
pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // HTML pages
        .route("/", get(handlers::dashboard))
        .route("/scenarios", get(handlers::scenarios_page))
        .route("/scenarios/:name", get(handlers::scenario_detail))
        .route("/results", get(handlers::results_page))
        .route("/results/:id", get(handlers::result_detail))
        .route("/run", get(handlers::run_page))
        .route("/load-test", get(handlers::load_test_page))
        .route("/targets", get(handlers::targets_page))
        // API endpoints
        .route("/api/scenarios", get(api::list_scenarios))
        .route("/api/scenarios/:name", get(api::get_scenario))
        .route("/api/run", post(api::run_scenario))
        .route("/api/status", get(api::get_status))
        .route("/api/results", get(api::list_results))
        .route("/api/results/:id", get(api::get_result))
        .route("/api/stop", post(api::stop_test))
        // Load test API
        .route("/api/load-test/start", post(api::start_load_test))
        .route("/api/load-test/stop", post(api::stop_load_test))
        .route("/api/load-test/status", get(api::load_test_status))
        .route("/api/targets", get(api::list_targets))
        .route("/api/targets", post(api::add_target))
        .route(
            "/api/targets/:id",
            axum::routing::delete(api::delete_target),
        )
        // Health check
        .route("/health", get(api::health_check))
        .layer(cors)
        .with_state(state)
}

/// Start the web server
pub async fn serve(config: WebConfig) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config.clone()));
    let app = create_router(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    info!("Starting Chaos Dashboard at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

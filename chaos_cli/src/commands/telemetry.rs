use anyhow::Result;
use axum::{
    extract::State,
    http::{header, HeaderValue},
    response::IntoResponse,
    routing::get,
    Router,
};
use chaos_metrics::exporters::prometheus::PrometheusExporter;
use chaos_scenarios::runner::RunTelemetry;
use std::{net::SocketAddr, sync::Arc};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct PrometheusServer {
    pub address: SocketAddr,
    cancellation: CancellationToken,
    task: JoinHandle<()>,
}

impl PrometheusServer {
    pub async fn start(port: u16, telemetry: Arc<RunTelemetry>) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        let address = listener.local_addr()?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let router = Router::new()
            .route("/metrics", get(metrics))
            .with_state(telemetry);
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await;
        });
        Ok(Self {
            address,
            cancellation,
            task,
        })
    }

    pub async fn shutdown(self) -> Result<()> {
        self.cancellation.cancel();
        self.task.await?;
        Ok(())
    }
}

async fn metrics(State(telemetry): State<Arc<RunTelemetry>>) -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    (
        headers,
        PrometheusExporter::format_run(&telemetry.snapshot()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_scrapeable_prometheus_metrics() {
        let server = PrometheusServer::start(0, Arc::new(RunTelemetry::default()))
            .await
            .unwrap();
        let response = reqwest::get(format!("http://{}/metrics", server.address))
            .await
            .unwrap();
        assert!(response.status().is_success());
        assert!(response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("text/plain"));
        assert!(response.text().await.unwrap().contains("chaos_run_active"));
        server.shutdown().await.unwrap();
    }
}

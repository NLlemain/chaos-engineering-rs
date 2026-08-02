//! Load testing module for custom applications, APIs, and streams

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Target type for load testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TargetType {
    Http,
    Tcp,
    Websocket,
    Rtmp,
    Hls,
    Grpc,
    Custom,
}

/// Load test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestConfig {
    pub name: String,
    pub target_type: TargetType,
    pub url: String,
    pub method: Option<String>, // For HTTP: GET, POST, etc.
    pub headers: Option<Vec<(String, String)>>,
    pub body: Option<String>,
    pub concurrent_users: u32,
    pub requests_per_second: u32,
    pub duration_secs: u64,
    pub timeout_ms: u64,
    pub ramp_up_secs: Option<u64>,
}

impl Default for LoadTestConfig {
    fn default() -> Self {
        Self {
            name: "New Load Test".to_string(),
            target_type: TargetType::Http,
            url: "http://localhost:3000".to_string(),
            method: Some("GET".to_string()),
            headers: None,
            body: None,
            concurrent_users: 10,
            requests_per_second: 100,
            duration_secs: 60,
            timeout_ms: 5000,
            ramp_up_secs: Some(10),
        }
    }
}

/// Real-time metrics during load test
#[derive(Debug, Clone, Serialize, Default)]
pub struct LoadTestMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub total_bytes: u64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub avg_latency_ms: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub requests_per_second: f64,
    pub errors: Vec<String>,
}

/// Load test state
pub struct LoadTestState {
    pub is_running: AtomicBool,
    pub should_stop: AtomicBool,
    pub config: RwLock<Option<LoadTestConfig>>,
    pub metrics: RwLock<LoadTestMetrics>,
    pub latencies: RwLock<Vec<f64>>,
    pub start_time: RwLock<Option<Instant>>,
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub total_bytes: AtomicU64,
    pub errors: RwLock<Vec<String>>,
}

impl Default for LoadTestState {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadTestState {
    pub fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
            should_stop: AtomicBool::new(false),
            config: RwLock::new(None),
            metrics: RwLock::new(LoadTestMetrics::default()),
            latencies: RwLock::new(Vec::new()),
            start_time: RwLock::new(None),
            total_requests: AtomicU64::new(0),
            successful_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            errors: RwLock::new(Vec::new()),
        }
    }

    pub async fn reset(&self) {
        self.total_requests.store(0, Ordering::SeqCst);
        self.successful_requests.store(0, Ordering::SeqCst);
        self.failed_requests.store(0, Ordering::SeqCst);
        self.total_bytes.store(0, Ordering::SeqCst);
        *self.metrics.write().await = LoadTestMetrics::default();
        self.latencies.write().await.clear();
        self.errors.write().await.clear();
    }
}

/// Run HTTP load test
pub async fn run_http_load_test(
    state: Arc<LoadTestState>,
    config: LoadTestConfig,
) -> anyhow::Result<LoadTestMetrics> {
    validate_config(&config)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()?;

    state.reset().await;
    state.is_running.store(true, Ordering::SeqCst);
    state.should_stop.store(false, Ordering::SeqCst);
    *state.config.write().await = Some(config.clone());
    *state.start_time.write().await = Some(Instant::now());

    let duration = Duration::from_secs(config.duration_secs);
    let ramp_up = Duration::from_secs(config.ramp_up_secs.unwrap_or(0));
    let deadline = tokio::time::Instant::now() + duration;
    let request_interval = Duration::from_secs_f64(1.0 / config.requests_per_second as f64);
    let mut interval = tokio::time::interval(request_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let rate_limiter = Arc::new(Mutex::new(interval));

    let mut workers = Vec::with_capacity(config.concurrent_users as usize);
    for user_index in 0..config.concurrent_users {
        let client = client.clone();
        let config = config.clone();
        let state = state.clone();
        let rate_limiter = rate_limiter.clone();
        let start_delay = if ramp_up.is_zero() {
            Duration::ZERO
        } else {
            ramp_up.mul_f64(user_index as f64 / config.concurrent_users as f64)
        };

        workers.push(tokio::spawn(async move {
            tokio::time::sleep(start_delay).await;
            loop {
                if state.should_stop.load(Ordering::SeqCst)
                    || tokio::time::Instant::now() >= deadline
                {
                    break;
                }

                rate_limiter.lock().await.tick().await;
                if state.should_stop.load(Ordering::SeqCst)
                    || tokio::time::Instant::now() >= deadline
                {
                    break;
                }
                make_request(&client, &config, &state).await;
            }
        }));
    }

    for worker in workers {
        if let Err(error) = worker.await {
            record_error(&state, format!("Load worker failed: {}", error)).await;
        }
    }

    state.is_running.store(false, Ordering::SeqCst);
    update_metrics(&state).await;

    Ok(state.metrics.read().await.clone())
}

pub(crate) fn validate_config(config: &LoadTestConfig) -> anyhow::Result<()> {
    if !matches!(config.target_type, TargetType::Http | TargetType::Hls) {
        anyhow::bail!("Only HTTP/HTTPS and HLS targets are currently supported");
    }
    if config.concurrent_users == 0 {
        anyhow::bail!("concurrent_users must be greater than zero");
    }
    if config.requests_per_second == 0 || config.requests_per_second > 1_000_000 {
        anyhow::bail!("requests_per_second must be between 1 and 1,000,000");
    }
    if config.duration_secs == 0 || config.timeout_ms == 0 {
        anyhow::bail!("duration_secs and timeout_ms must be greater than zero");
    }
    if config.ramp_up_secs.unwrap_or(0) > config.duration_secs {
        anyhow::bail!("ramp_up_secs cannot exceed duration_secs");
    }

    let url = reqwest::Url::parse(&config.url)?;
    if !matches!(url.scheme(), "http" | "https") {
        anyhow::bail!("Load test URL must use http or https");
    }
    reqwest::Method::from_bytes(config.method.as_deref().unwrap_or("GET").as_bytes())?;
    Ok(())
}

async fn make_request(
    client: &reqwest::Client,
    config: &LoadTestConfig,
    state: &Arc<LoadTestState>,
) {
    let start = Instant::now();
    let method = reqwest::Method::from_bytes(config.method.as_deref().unwrap_or("GET").as_bytes())
        .unwrap_or(reqwest::Method::GET);
    let mut request = client.request(method, &config.url);

    if let Some(headers) = &config.headers {
        for (key, value) in headers {
            request = request.header(key, value);
        }
    }

    if let Some(body) = &config.body {
        request = request.body(body.clone());
    }

    state.total_requests.fetch_add(1, Ordering::SeqCst);

    match request.send().await {
        Ok(response) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            state.latencies.write().await.push(latency);
            let status = response.status();

            match response.bytes().await {
                Ok(body) => {
                    state
                        .total_bytes
                        .fetch_add(body.len() as u64, Ordering::SeqCst);
                }
                Err(error) => {
                    state.failed_requests.fetch_add(1, Ordering::SeqCst);
                    record_error(state, format!("Failed to read response body: {}", error)).await;
                    return;
                }
            }

            if status.is_success() {
                state.successful_requests.fetch_add(1, Ordering::SeqCst);
            } else {
                state.failed_requests.fetch_add(1, Ordering::SeqCst);
                record_error(state, format!("HTTP response status: {}", status)).await;
            }
        }
        Err(error) => {
            state
                .latencies
                .write()
                .await
                .push(start.elapsed().as_secs_f64() * 1000.0);
            state.failed_requests.fetch_add(1, Ordering::SeqCst);
            record_error(state, format!("Request failed: {}", error)).await;
        }
    }
}

async fn record_error(state: &Arc<LoadTestState>, error: String) {
    const MAX_RECORDED_ERRORS: usize = 20;
    let mut errors = state.errors.write().await;
    if errors.len() < MAX_RECORDED_ERRORS && !errors.contains(&error) {
        errors.push(error);
    }
}

async fn update_metrics(state: &Arc<LoadTestState>) {
    let latencies = state.latencies.read().await;
    let total = state.total_requests.load(Ordering::SeqCst);
    let successful = state.successful_requests.load(Ordering::SeqCst);
    let failed = state.failed_requests.load(Ordering::SeqCst);
    let total_bytes = state.total_bytes.load(Ordering::SeqCst);
    let errors = state.errors.read().await.clone();

    let start_time = state.start_time.read().await;
    let elapsed = start_time.map(|s| s.elapsed().as_secs_f64()).unwrap_or(1.0);

    let mut sorted_latencies: Vec<f64> = latencies.clone();
    sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let metrics = LoadTestMetrics {
        total_requests: total,
        successful_requests: successful,
        failed_requests: failed,
        total_bytes,
        min_latency_ms: sorted_latencies.first().copied().unwrap_or(0.0),
        max_latency_ms: sorted_latencies.last().copied().unwrap_or(0.0),
        avg_latency_ms: if !sorted_latencies.is_empty() {
            sorted_latencies.iter().sum::<f64>() / sorted_latencies.len() as f64
        } else {
            0.0
        },
        p50_latency_ms: percentile(&sorted_latencies, 50.0),
        p95_latency_ms: percentile(&sorted_latencies, 95.0),
        p99_latency_ms: percentile(&sorted_latencies, 99.0),
        requests_per_second: total as f64 / elapsed,
        errors,
    };

    *state.metrics.write().await = metrics;
}

fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }
    let idx = ((p / 100.0) * (sorted_data.len() - 1) as f64) as usize;
    sorted_data[idx.min(sorted_data.len() - 1)]
}

/// Stream test for video/audio pipelines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamTestConfig {
    pub name: String,
    pub stream_url: String,
    pub stream_type: StreamType,
    pub duration_secs: u64,
    pub viewers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Hls,
    Rtmp,
    WebRtc,
    Dash,
    Srt,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct StreamTestMetrics {
    pub total_viewers: u32,
    pub connected_viewers: u32,
    pub buffering_events: u64,
    pub avg_bitrate_kbps: f64,
    pub avg_latency_ms: f64,
    pub dropped_frames: u64,
    pub total_bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};

    #[test]
    fn rejects_unsupported_target_type() {
        let config = LoadTestConfig {
            target_type: TargetType::Tcp,
            ..LoadTestConfig::default()
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn rejects_invalid_rate_and_ramp_up() {
        let zero_rate = LoadTestConfig {
            requests_per_second: 0,
            ..LoadTestConfig::default()
        };
        assert!(validate_config(&zero_rate).is_err());

        let excessive_ramp_up = LoadTestConfig {
            duration_secs: 10,
            ramp_up_secs: Some(11),
            ..LoadTestConfig::default()
        };
        assert!(validate_config(&excessive_ramp_up).is_err());
    }

    #[tokio::test]
    async fn sends_repeated_paced_requests_and_collects_bytes() {
        let app = Router::new().route("/", get(|| async { "hello" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let state = Arc::new(LoadTestState::new());
        let config = LoadTestConfig {
            name: "paced test".to_string(),
            url: format!("http://{}/", address),
            concurrent_users: 4,
            requests_per_second: 20,
            duration_secs: 1,
            timeout_ms: 1_000,
            ramp_up_secs: Some(0),
            ..LoadTestConfig::default()
        };

        let metrics = run_http_load_test(state, config).await.unwrap();
        server.abort();

        assert!(metrics.total_requests >= 10, "{metrics:?}");
        assert_eq!(metrics.successful_requests, metrics.total_requests);
        assert_eq!(metrics.failed_requests, 0);
        assert!(metrics.total_bytes >= metrics.total_requests * 5);
    }
}

//! Load testing module for custom applications, APIs, and streams

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

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
        }
    }

    pub fn reset(&self) {
        self.total_requests.store(0, Ordering::SeqCst);
        self.successful_requests.store(0, Ordering::SeqCst);
        self.failed_requests.store(0, Ordering::SeqCst);
    }
}

/// Run HTTP load test
pub async fn run_http_load_test(
    state: Arc<LoadTestState>,
    config: LoadTestConfig,
) -> anyhow::Result<LoadTestMetrics> {
    state.is_running.store(true, Ordering::SeqCst);
    state.should_stop.store(false, Ordering::SeqCst);
    state.reset();

    *state.config.write().await = Some(config.clone());
    *state.start_time.write().await = Some(Instant::now());
    *state.latencies.write().await = Vec::new();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(config.timeout_ms))
        .build()?;

    let duration = Duration::from_secs(config.duration_secs);
    let start = Instant::now();
    let ramp_up = Duration::from_secs(config.ramp_up_secs.unwrap_or(0));

    let delay_between_requests = if config.requests_per_second > 0 {
        Duration::from_micros(1_000_000 / config.requests_per_second as u64)
    } else {
        Duration::from_millis(10)
    };

    let mut handles = Vec::new();
    let mut current_users = 0u32;
    let target_users = config.concurrent_users;

    while start.elapsed() < duration && !state.should_stop.load(Ordering::SeqCst) {
        // Ramp up users
        let elapsed_ratio = if ramp_up.as_secs() > 0 {
            (start.elapsed().as_secs_f64() / ramp_up.as_secs_f64()).min(1.0)
        } else {
            1.0
        };
        let desired_users = ((target_users as f64) * elapsed_ratio) as u32;

        while current_users < desired_users && current_users < target_users {
            let client = client.clone();
            let config = config.clone();
            let state = state.clone();

            let handle = tokio::spawn(async move { make_request(&client, &config, &state).await });
            handles.push(handle);
            current_users += 1;
        }

        // Update metrics periodically
        update_metrics(&state).await;

        tokio::time::sleep(delay_between_requests).await;
    }

    // Wait for all requests to complete
    for handle in handles {
        let _ = handle.await;
    }

    state.is_running.store(false, Ordering::SeqCst);

    // Final metrics update
    update_metrics(&state).await;

    Ok(state.metrics.read().await.clone())
}

async fn make_request(
    client: &reqwest::Client,
    config: &LoadTestConfig,
    state: &Arc<LoadTestState>,
) {
    let start = Instant::now();

    let mut request = match config.method.as_deref().unwrap_or("GET") {
        "POST" => client.post(&config.url),
        "PUT" => client.put(&config.url),
        "DELETE" => client.delete(&config.url),
        "PATCH" => client.patch(&config.url),
        _ => client.get(&config.url),
    };

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

            if response.status().is_success() {
                state.successful_requests.fetch_add(1, Ordering::SeqCst);
            } else {
                state.failed_requests.fetch_add(1, Ordering::SeqCst);
            }
        }
        Err(_e) => {
            state.failed_requests.fetch_add(1, Ordering::SeqCst);
        }
    }
}

async fn update_metrics(state: &Arc<LoadTestState>) {
    let latencies = state.latencies.read().await;
    let total = state.total_requests.load(Ordering::SeqCst);
    let successful = state.successful_requests.load(Ordering::SeqCst);
    let failed = state.failed_requests.load(Ordering::SeqCst);

    let start_time = state.start_time.read().await;
    let elapsed = start_time.map(|s| s.elapsed().as_secs_f64()).unwrap_or(1.0);

    let mut sorted_latencies: Vec<f64> = latencies.clone();
    sorted_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let metrics = LoadTestMetrics {
        total_requests: total,
        successful_requests: successful,
        failed_requests: failed,
        total_bytes: 0,
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
        errors: Vec::new(),
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

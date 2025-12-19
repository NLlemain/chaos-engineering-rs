//! Application state management

use crate::load_test::LoadTestState;
use crate::WebConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

/// Current test execution status
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestStatus {
    pub is_running: bool,
    pub scenario_name: Option<String>,
    pub current_phase: Option<String>,
    pub progress_percent: f32,
    pub elapsed_seconds: u64,
    pub total_seconds: u64,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for TestStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            scenario_name: None,
            current_phase: None,
            progress_percent: 0.0,
            elapsed_seconds: 0,
            total_seconds: 0,
            started_at: None,
        }
    }
}

/// Shared application state
pub struct AppState {
    pub config: WebConfig,
    pub test_status: RwLock<TestStatus>,
    pub stop_signal: AtomicBool,
    pub recent_results: RwLock<Vec<ResultSummary>>,
    pub load_test_state: Arc<LoadTestState>,
    pub custom_targets: RwLock<Vec<CustomTarget>>,
}

/// Custom target for stress testing
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CustomTarget {
    pub id: String,
    pub name: String,
    pub target_type: String,
    pub url: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Summary of a test result for listing
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResultSummary {
    pub id: String,
    pub scenario_name: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success_rate: f64,
    pub total_duration_secs: u64,
    pub file_path: PathBuf,
}

impl AppState {
    pub fn new(config: WebConfig) -> Self {
        Self {
            config,
            test_status: RwLock::new(TestStatus::default()),
            stop_signal: AtomicBool::new(false),
            recent_results: RwLock::new(Vec::new()),
            load_test_state: Arc::new(LoadTestState::new()),
            custom_targets: RwLock::new(Vec::new()),
        }
    }

    pub fn get_status(&self) -> TestStatus {
        self.test_status.read().unwrap().clone()
    }

    pub fn update_status(&self, status: TestStatus) {
        *self.test_status.write().unwrap() = status;
    }

    pub fn set_running(&self, scenario_name: String, total_seconds: u64) {
        let mut status = self.test_status.write().unwrap();
        status.is_running = true;
        status.scenario_name = Some(scenario_name);
        status.total_seconds = total_seconds;
        status.elapsed_seconds = 0;
        status.progress_percent = 0.0;
        status.started_at = Some(chrono::Utc::now());
        self.stop_signal.store(false, Ordering::SeqCst);
    }

    pub fn set_stopped(&self) {
        let mut status = self.test_status.write().unwrap();
        status.is_running = false;
        status.current_phase = None;
    }

    pub fn update_progress(&self, phase: &str, elapsed: u64) {
        let mut status = self.test_status.write().unwrap();
        status.current_phase = Some(phase.to_string());
        status.elapsed_seconds = elapsed;
        if status.total_seconds > 0 {
            status.progress_percent = (elapsed as f32 / status.total_seconds as f32) * 100.0;
        }
    }

    pub fn should_stop(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }

    pub fn request_stop(&self) {
        self.stop_signal.store(true, Ordering::SeqCst);
    }

    pub fn add_result(&self, summary: ResultSummary) {
        let mut results = self.recent_results.write().unwrap();
        results.insert(0, summary);
        // Keep only last 50 results in memory
        if results.len() > 50 {
            results.truncate(50);
        }
    }

    pub fn get_recent_results(&self) -> Vec<ResultSummary> {
        self.recent_results.read().unwrap().clone()
    }
}

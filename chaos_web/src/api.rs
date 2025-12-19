//! REST API endpoints

use crate::load_test::{run_http_load_test, LoadTestConfig, LoadTestMetrics};
use crate::state::{AppState, CustomTarget, ResultSummary};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Health check endpoint
pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Get current test status
pub async fn get_status(State(state): State<Arc<AppState>>) -> Json<crate::state::TestStatus> {
    Json(state.get_status())
}

/// Scenario info response
#[derive(Serialize)]
pub struct ScenarioResponse {
    pub file_name: String,
    pub name: String,
    pub description: Option<String>,
    pub duration_secs: u64,
    pub phase_count: usize,
    pub phases: Vec<PhaseResponse>,
}

/// Phase info response
#[derive(Serialize)]
pub struct PhaseResponse {
    pub name: String,
    pub duration_secs: u64,
    pub injections: Vec<String>,
}

/// List all available scenarios
pub async fn list_scenarios(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ScenarioResponse>>, (StatusCode, String)> {
    let mut scenarios = Vec::new();

    let mut entries = tokio::fs::read_dir(&state.config.scenarios_dir)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path
            .extension()
            .map_or(false, |e| e == "yaml" || e == "yml")
        {
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                if let Ok(scenario) = chaos_scenarios::parse_scenario(&content) {
                    let file_name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown.yaml")
                        .to_string();

                    let phases: Vec<PhaseResponse> = scenario
                        .phases
                        .iter()
                        .map(|p| PhaseResponse {
                            name: p.name.clone(),
                            duration_secs: p.duration.as_secs(),
                            injections: p.injections.iter().map(|i| i.r#type.clone()).collect(),
                        })
                        .collect();

                    scenarios.push(ScenarioResponse {
                        file_name,
                        name: scenario.name,
                        description: scenario.description,
                        duration_secs: scenario.duration.as_secs(),
                        phase_count: scenario.phases.len(),
                        phases,
                    });
                }
            }
        }
    }

    scenarios.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(scenarios))
}

/// Get a specific scenario
pub async fn get_scenario(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ScenarioResponse>, (StatusCode, String)> {
    let scenario_path = state.config.scenarios_dir.join(&name);

    let content = tokio::fs::read_to_string(&scenario_path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Scenario not found: {}", e)))?;

    let scenario = chaos_scenarios::parse_scenario(&content)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid scenario: {}", e)))?;

    let phases: Vec<PhaseResponse> = scenario
        .phases
        .iter()
        .map(|p| PhaseResponse {
            name: p.name.clone(),
            duration_secs: p.duration.as_secs(),
            injections: p.injections.iter().map(|i| i.r#type.clone()).collect(),
        })
        .collect();

    Ok(Json(ScenarioResponse {
        file_name: name,
        name: scenario.name,
        description: scenario.description,
        duration_secs: scenario.duration.as_secs(),
        phase_count: scenario.phases.len(),
        phases,
    }))
}

/// Run scenario request
#[derive(Deserialize)]
pub struct RunRequest {
    pub scenario: String,
}

/// Run scenario response
#[derive(Serialize)]
pub struct RunResponse {
    pub success: bool,
    pub message: String,
}

/// Run a chaos test scenario
pub async fn run_scenario(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RunRequest>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    // Check if a test is already running
    if state.get_status().is_running {
        return Err((
            StatusCode::CONFLICT,
            "A test is already running".to_string(),
        ));
    }

    let scenario_path = state.config.scenarios_dir.join(&request.scenario);

    // Parse scenario
    let content = tokio::fs::read_to_string(&scenario_path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Scenario not found: {}", e)))?;

    let scenario = chaos_scenarios::parse_scenario(&content)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid scenario: {}", e)))?;

    let total_seconds = scenario.duration.as_secs();
    let scenario_name = scenario.name.clone();
    let scenario_name_for_response = scenario_name.clone();

    // Set running state
    state.set_running(scenario_name.clone(), total_seconds);

    // Spawn test runner
    let state_clone = state.clone();
    let results_dir = state.config.results_dir.clone();

    tokio::spawn(async move {
        let runner = chaos_scenarios::ScenarioRunner::with_defaults();

        // Simulate progress updates
        let phases = scenario.phases.clone();
        let state_for_progress = state_clone.clone();
        let duration = scenario.duration;

        tokio::spawn(async move {
            let start = tokio::time::Instant::now();
            let mut current_phase_idx = 0;
            loop {
                if state_for_progress.should_stop() {
                    break;
                }

                let elapsed = start.elapsed();
                if elapsed >= duration {
                    break;
                }

                // Determine current phase
                let mut cumulative = std::time::Duration::ZERO;
                for (idx, phase) in phases.iter().enumerate() {
                    cumulative += phase.duration;
                    if elapsed < cumulative {
                        current_phase_idx = idx;
                        break;
                    }
                }

                if let Some(phase) = phases.get(current_phase_idx) {
                    state_for_progress.update_progress(&phase.name, elapsed.as_secs());
                }

                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });

        // Run the actual scenario
        match runner.run(&scenario).await {
            Ok(result) => {
                // Save result
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
                let result_file = results_dir.join(format!(
                    "{}_{}.json",
                    scenario_name.replace(' ', "_").to_lowercase(),
                    timestamp
                ));

                if let Ok(json) = serde_json::to_string_pretty(&result) {
                    let _ = tokio::fs::write(&result_file, json).await;
                }

                // Add to recent results
                state_clone.add_result(ResultSummary {
                    id: format!(
                        "{}_{}",
                        scenario_name.replace(' ', "_").to_lowercase(),
                        timestamp
                    ),
                    scenario_name: result.scenario_name.clone(),
                    timestamp: result.started_at,
                    success_rate: result.success_rate(),
                    total_duration_secs: result.total_duration.as_secs(),
                    file_path: result_file,
                });
            }
            Err(e) => {
                tracing::error!("Scenario execution failed: {}", e);
            }
        }

        state_clone.set_stopped();
    });

    Ok(Json(RunResponse {
        success: true,
        message: format!("Started test: {}", scenario_name_for_response),
    }))
}

/// Stop the current test
pub async fn stop_test(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    if !state.get_status().is_running {
        return Err((
            StatusCode::BAD_REQUEST,
            "No test is currently running".to_string(),
        ));
    }

    state.request_stop();
    state.set_stopped();

    Ok(Json(RunResponse {
        success: true,
        message: "Test stopped".to_string(),
    }))
}

/// List test results
pub async fn list_results(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ResultSummary>>, (StatusCode, String)> {
    let mut results = state.get_recent_results();

    // Also load results from disk
    if let Ok(mut entries) = tokio::fs::read_dir(&state.config.results_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                    if let Ok(result) =
                        serde_json::from_str::<chaos_scenarios::runner::ScenarioResult>(&content)
                    {
                        let file_name = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_string();

                        // Check if already in results
                        if !results.iter().any(|r| r.id == file_name) {
                            results.push(ResultSummary {
                                id: file_name,
                                scenario_name: result.scenario_name.clone(),
                                timestamp: result.started_at,
                                success_rate: result.success_rate(),
                                total_duration_secs: result.total_duration.as_secs(),
                                file_path: path,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp descending
    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(Json(results))
}

/// Get a specific result
pub async fn get_result(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<chaos_scenarios::runner::ScenarioResult>, (StatusCode, String)> {
    let result_path = state.config.results_dir.join(format!("{}.json", id));

    let content = tokio::fs::read_to_string(&result_path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Result not found: {}", e)))?;

    let result: chaos_scenarios::runner::ScenarioResult =
        serde_json::from_str(&content).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid result format: {}", e),
            )
        })?;

    Ok(Json(result))
}

// ============== Load Test API ==============

/// Start a load test
pub async fn start_load_test(
    State(state): State<Arc<AppState>>,
    Json(config): Json<LoadTestConfig>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    if state.load_test_state.is_running.load(Ordering::SeqCst) {
        return Err((
            StatusCode::CONFLICT,
            "A load test is already running".to_string(),
        ));
    }

    let load_state = state.load_test_state.clone();

    tokio::spawn(async move {
        let _ = run_http_load_test(load_state, config).await;
    });

    Ok(Json(RunResponse {
        success: true,
        message: "Load test started".to_string(),
    }))
}

/// Stop the current load test
pub async fn stop_load_test(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    if !state.load_test_state.is_running.load(Ordering::SeqCst) {
        return Err((
            StatusCode::BAD_REQUEST,
            "No load test is running".to_string(),
        ));
    }

    state
        .load_test_state
        .should_stop
        .store(true, Ordering::SeqCst);

    Ok(Json(RunResponse {
        success: true,
        message: "Load test stopping".to_string(),
    }))
}

/// Load test status response
#[derive(Serialize)]
pub struct LoadTestStatusResponse {
    pub is_running: bool,
    pub config: Option<LoadTestConfig>,
    pub metrics: LoadTestMetrics,
}

/// Get load test status
pub async fn load_test_status(State(state): State<Arc<AppState>>) -> Json<LoadTestStatusResponse> {
    let is_running = state.load_test_state.is_running.load(Ordering::SeqCst);
    let config = state.load_test_state.config.read().await.clone();
    let metrics = state.load_test_state.metrics.read().await.clone();

    Json(LoadTestStatusResponse {
        is_running,
        config,
        metrics,
    })
}

// ============== Custom Targets API ==============

/// List custom targets
pub async fn list_targets(State(state): State<Arc<AppState>>) -> Json<Vec<CustomTarget>> {
    Json(state.custom_targets.read().unwrap().clone())
}

/// Add target request
#[derive(Deserialize)]
pub struct AddTargetRequest {
    pub name: String,
    pub target_type: String,
    pub url: String,
    pub description: Option<String>,
}

/// Add a custom target
pub async fn add_target(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AddTargetRequest>,
) -> Result<Json<CustomTarget>, (StatusCode, String)> {
    let target = CustomTarget {
        id: uuid_simple(),
        name: request.name,
        target_type: request.target_type,
        url: request.url,
        description: request.description,
        created_at: chrono::Utc::now(),
    };

    state.custom_targets.write().unwrap().push(target.clone());

    Ok(Json(target))
}

/// Delete a custom target
pub async fn delete_target(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RunResponse>, (StatusCode, String)> {
    let mut targets = state.custom_targets.write().unwrap();
    let initial_len = targets.len();
    targets.retain(|t| t.id != id);

    if targets.len() == initial_len {
        return Err((StatusCode::NOT_FOUND, "Target not found".to_string()));
    }

    Ok(Json(RunResponse {
        success: true,
        message: "Target deleted".to_string(),
    }))
}

/// Simple UUID generator (no external dependency)
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}

//! HTML page handlers

use crate::state::AppState;
use crate::templates::{self, PhaseInfo, ScenarioInfo};
use axum::{
    extract::{Path, State},
    response::Html,
};
use std::sync::Arc;

/// Dashboard home page
pub async fn dashboard(State(state): State<Arc<AppState>>) -> Html<String> {
    let scenarios = load_scenarios(&state).await;
    let results = state.get_recent_results();
    let status = state.get_status();

    Html(templates::dashboard_page(
        scenarios.len(),
        results.len(),
        &results,
        &status,
    ))
}

/// Scenarios list page
pub async fn scenarios_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let scenarios = load_scenarios(&state).await;
    Html(templates::scenarios_page(&scenarios))
}

/// Scenario detail page
pub async fn scenario_detail(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Html<String> {
    let scenario_path = state.config.scenarios_dir.join(&name);

    match tokio::fs::read_to_string(&scenario_path).await {
        Ok(yaml_content) => match chaos_scenarios::parse_scenario(&yaml_content) {
            Ok(scenario) => {
                let info = ScenarioInfo {
                    file_name: name.clone(),
                    name: scenario.name.clone(),
                    description: scenario.description.clone(),
                    duration: format!("{}s", scenario.duration.as_secs()),
                    phase_count: scenario.phases.len(),
                };

                let phases: Vec<PhaseInfo> = scenario
                    .phases
                    .iter()
                    .map(|p| PhaseInfo {
                        name: p.name.clone(),
                        duration: format!("{}s", p.duration.as_secs()),
                        injections: p.injections.iter().map(|i| i.r#type.clone()).collect(),
                    })
                    .collect();

                Html(templates::scenario_detail_page(
                    &info,
                    &yaml_content,
                    &phases,
                ))
            }
            Err(e) => Html(templates::error_page("Parse Error", &e.to_string())),
        },
        Err(e) => Html(templates::error_page(
            "Not Found",
            &format!("Scenario '{}' not found: {}", name, e),
        )),
    }
}

/// Results list page
pub async fn results_page(State(state): State<Arc<AppState>>) -> Html<String> {
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
                            results.push(crate::state::ResultSummary {
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

    Html(templates::results_page(&results))
}

/// Result detail page
pub async fn result_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Html<String> {
    // Try to find result file
    let result_path = state.config.results_dir.join(format!("{}.json", id));

    match tokio::fs::read_to_string(&result_path).await {
        Ok(content) => {
            match serde_json::from_str::<chaos_scenarios::runner::ScenarioResult>(&content) {
                Ok(result) => Html(templates::result_detail_page(&result)),
                Err(e) => Html(templates::error_page("Parse Error", &e.to_string())),
            }
        }
        Err(e) => Html(templates::error_page(
            "Not Found",
            &format!("Result '{}' not found: {}", id, e),
        )),
    }
}

/// Run test page
pub async fn run_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let scenarios = load_scenarios(&state).await;
    let status = state.get_status();
    Html(templates::run_page(&scenarios, &status))
}

/// Load test page
pub async fn load_test_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let targets = state.custom_targets.read().unwrap().clone();
    let is_running = state
        .load_test_state
        .is_running
        .load(std::sync::atomic::Ordering::SeqCst);
    let metrics = state.load_test_state.metrics.read().await.clone();
    Html(templates::load_test_page(&targets, is_running, &metrics))
}

/// Custom targets page
pub async fn targets_page(State(state): State<Arc<AppState>>) -> Html<String> {
    let targets = state.custom_targets.read().unwrap().clone();
    Html(templates::targets_page(&targets))
}

/// Load all scenarios from the scenarios directory
async fn load_scenarios(state: &AppState) -> Vec<ScenarioInfo> {
    let mut scenarios = Vec::new();

    if let Ok(mut entries) = tokio::fs::read_dir(&state.config.scenarios_dir).await {
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

                        scenarios.push(ScenarioInfo {
                            file_name,
                            name: scenario.name,
                            description: scenario.description,
                            duration: format!("{}s", scenario.duration.as_secs()),
                            phase_count: scenario.phases.len(),
                        });
                    }
                }
            }
        }
    }

    scenarios.sort_by(|a, b| a.name.cmp(&b.name));
    scenarios
}

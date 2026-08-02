use assert_cmd::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn failing_slo_returns_nonzero_after_writing_report() {
    let directory = std::env::temp_dir().join(format!(
        "chaos-slo-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let scenario = directory.join("failure.yaml");
    let report = directory.join("result.json");
    std::fs::write(
        &scenario,
        r#"name: failing_gate
duration: 180ms
phases:
  - name: observe
    duration: 180ms
    injections: []
assertions:
  - name: unavailable_service
    url: http://127.0.0.1:1/health
    interval: 20ms
    timeout: 20ms
    max_error_rate: 0.0
    min_requests: 2
"#,
    )
    .unwrap();

    Command::cargo_bin("chaos")
        .unwrap()
        .args([
            "run",
            scenario.to_str().unwrap(),
            "--output-json",
            report.to_str().unwrap(),
        ])
        .assert()
        .failure();

    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!(result["slo_results"][0]["passed"], false);
    std::fs::remove_dir_all(directory).unwrap();
}

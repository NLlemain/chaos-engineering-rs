use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

pub async fn execute(
    metrics_file: PathBuf,
    format: String,
    output: Option<PathBuf>,
    compare: Vec<PathBuf>,
) -> Result<()> {
    println!("{}", "=== Generate Report ===".bold().cyan());
    println!("Metrics file: {}", metrics_file.display());
    println!("Format: {}", format);

    // Load metrics
    let contents = tokio::fs::read_to_string(&metrics_file).await?;
    let result: chaos_scenarios::runner::ScenarioResult = serde_json::from_str(&contents)?;

    match format.as_str() {
        "cli" => {
            print_cli_report(&result);
        }
        "json" => {
            let json = serde_json::to_string_pretty(&result)?;
            if let Some(output_path) = output {
                tokio::fs::write(output_path, json).await?;
            } else {
                println!("{}", json);
            }
        }
        "markdown" => {
            let md = generate_markdown_report(&result);
            if let Some(output_path) = output {
                tokio::fs::write(output_path, md).await?;
            } else {
                println!("{}", md);
            }
        }
        "html" => {
            let html = generate_html_report(&result);
            if let Some(output_path) = output {
                tokio::fs::write(&output_path, html).await?;
                println!(
                    "{} HTML report generated: {}",
                    "✓".green(),
                    output_path.display()
                );
            } else {
                println!("{}", html);
            }
        }
        _ => {
            anyhow::bail!("Unknown format: {}", format);
        }
    }

    for baseline_path in compare {
        let baseline_contents = tokio::fs::read_to_string(&baseline_path).await?;
        let baseline: chaos_scenarios::runner::ScenarioResult =
            serde_json::from_str(&baseline_contents)?;
        print_comparison(&result, &baseline, &baseline_path);
    }

    Ok(())
}

fn print_cli_report(result: &chaos_scenarios::runner::ScenarioResult) {
    println!("\n{}", "=== Scenario Report ===".bold().green());
    println!("Scenario: {}", result.scenario_name.cyan());
    println!("Total Duration: {:?}", result.total_duration);
    println!("Total Injections: {}", result.total_injections);
    println!("Success Rate: {:.2}%", result.success_rate() * 100.0);

    println!("\n{}", "Phase Results:".bold());
    for phase in &result.phase_results {
        println!(
            "  {} - Duration: {:?}, Injections: {}",
            phase.name.yellow(),
            phase.duration,
            phase.injection_count
        );
    }
    for slo in &result.slo_results {
        println!(
            "  SLO {}: {} ({:.2}% errors, p95 {:?})",
            slo.name,
            if slo.passed { "PASS" } else { "FAIL" },
            slo.error_rate * 100.0,
            slo.latency_p95
        );
    }
}

fn print_comparison(
    chaos: &chaos_scenarios::runner::ScenarioResult,
    baseline: &chaos_scenarios::runner::ScenarioResult,
    baseline_path: &std::path::Path,
) {
    println!("\n{}", "=== Baseline Comparison ===".bold().cyan());
    println!("Baseline: {}", baseline_path.display());
    println!(
        "Injection success: {:.2}% -> {:.2}% ({:+.2} points)",
        baseline.success_rate() * 100.0,
        chaos.success_rate() * 100.0,
        (chaos.success_rate() - baseline.success_rate()) * 100.0
    );
    println!(
        "Duration: {:?} -> {:?} ({:.2}x)",
        baseline.total_duration,
        chaos.total_duration,
        duration_ratio(chaos.total_duration, baseline.total_duration)
    );
    println!(
        "Probe error rate: {:.2}% -> {:.2}% ({:+.2} points)",
        combined_slo_error_rate(baseline) * 100.0,
        combined_slo_error_rate(chaos) * 100.0,
        (combined_slo_error_rate(chaos) - combined_slo_error_rate(baseline)) * 100.0
    );
    println!(
        "SLO gate: {} -> {}",
        if baseline.slos_passed() {
            "PASS"
        } else {
            "FAIL"
        },
        if chaos.slos_passed() { "PASS" } else { "FAIL" }
    );
}

fn combined_slo_error_rate(result: &chaos_scenarios::runner::ScenarioResult) -> f64 {
    let requests: usize = result
        .slo_results
        .iter()
        .map(|slo| slo.total_requests)
        .sum();
    let failures: usize = result
        .slo_results
        .iter()
        .map(|slo| slo.failed_requests)
        .sum();
    if requests == 0 {
        0.0
    } else {
        failures as f64 / requests as f64
    }
}

fn duration_ratio(current: std::time::Duration, baseline: std::time::Duration) -> f64 {
    if baseline.is_zero() {
        0.0
    } else {
        current.as_secs_f64() / baseline.as_secs_f64()
    }
}

pub(crate) fn generate_markdown_report(result: &chaos_scenarios::runner::ScenarioResult) -> String {
    let slo_results = if result.slo_results.is_empty() {
        "- No SLO assertions configured".to_string()
    } else {
        result
            .slo_results
            .iter()
            .map(|slo| {
                format!(
                    "- **{}**: {} ({} probes, {:.2}% errors, p95 {:?})",
                    slo.name,
                    if slo.passed { "PASS" } else { "FAIL" },
                    slo.total_requests,
                    slo.error_rate * 100.0,
                    slo.latency_p95
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        r#"# Chaos Test Report: {}

## Summary

- **Total Duration**: {:?}
- **Total Injections**: {}
- **Success Rate**: {:.2}%

## Phase Results

{}

## SLO Assertions

{}

## Conclusion

{}
"#,
        result.scenario_name,
        result.total_duration,
        result.total_injections,
        result.success_rate() * 100.0,
        result
            .phase_results
            .iter()
            .map(|p| format!(
                "- **{}**: {:?} ({} injections)",
                p.name, p.duration, p.injection_count
            ))
            .collect::<Vec<_>>()
            .join("\n"),
        slo_results,
        if result.slos_passed() {
            "SLO gate passed."
        } else {
            "SLO gate failed."
        }
    )
}

pub(crate) fn generate_html_report(result: &chaos_scenarios::runner::ScenarioResult) -> String {
    let success_rate = result.success_rate() * 100.0;
    let success_class = if success_rate >= 90.0 {
        "success"
    } else if success_rate >= 70.0 {
        "warning"
    } else {
        "danger"
    };

    let phases_html: String = result
        .phase_results
        .iter()
        .map(|p| {
            format!(
                r#"<tr>
                    <td><strong>{}</strong></td>
                    <td>{:?}</td>
                    <td>{}</td>
                </tr>"#,
                p.name, p.duration, p.injection_count
            )
        })
        .collect();

    let (slo_gate, slo_class) = if result.slo_results.is_empty() {
        ("NOT CONFIGURED", "warning")
    } else if result.slos_passed() {
        ("PASS", "success")
    } else {
        ("FAIL", "danger")
    };
    let slo_results_html = if result.slo_results.is_empty() {
        r#"<tr><td colspan="5">No SLO assertions configured.</td></tr>"#.to_string()
    } else {
        result
            .slo_results
            .iter()
            .map(|slo| {
                format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td class="stat-value {}">{}</td>
                        <td>{}</td>
                        <td>{:.2}%</td>
                        <td>{:?}</td>
                    </tr>"#,
                    slo.name,
                    if slo.passed { "success" } else { "danger" },
                    if slo.passed { "PASS" } else { "FAIL" },
                    slo.total_requests,
                    slo.error_rate * 100.0,
                    slo.latency_p95
                )
            })
            .collect()
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Chaos Test Report - {scenario_name}</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        :root {{
            --bg-primary: #0d0d0d;
            --bg-secondary: #1a1a1a;
            --bg-tertiary: #2d2d2d;
            --text-primary: #ffffff;
            --text-secondary: #a0a0a0;
            --accent-blue: #3b82f6;
            --accent-green: #22c55e;
            --accent-red: #ef4444;
            --accent-yellow: #eab308;
            --border-color: #333333;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Arial, sans-serif;
            background-color: var(--bg-primary);
            color: var(--text-primary);
            line-height: 1.6;
            padding: 2rem;
        }}
        .container {{
            max-width: 1000px;
            margin: 0 auto;
        }}
        header {{
            text-align: center;
            margin-bottom: 3rem;
            padding-bottom: 2rem;
            border-bottom: 1px solid var(--border-color);
        }}
        h1 {{
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }}
        .subtitle {{
            color: var(--text-secondary);
            font-size: 1.1rem;
        }}
        .stats-grid {{
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: 1.5rem;
            margin-bottom: 3rem;
        }}
        .stat-card {{
            background-color: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.5rem;
            text-align: center;
        }}
        .stat-icon {{
            font-size: 2rem;
            margin-bottom: 0.5rem;
        }}
        .stat-value {{
            font-size: 2rem;
            font-weight: 700;
            margin-bottom: 0.25rem;
        }}
        .stat-value.success {{ color: var(--accent-green); }}
        .stat-value.warning {{ color: var(--accent-yellow); }}
        .stat-value.danger {{ color: var(--accent-red); }}
        .stat-label {{
            color: var(--text-secondary);
            font-size: 0.875rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }}
        .card {{
            background-color: var(--bg-secondary);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
        }}
        .card-title {{
            font-size: 1.25rem;
            font-weight: 600;
            margin-bottom: 1rem;
            padding-bottom: 0.75rem;
            border-bottom: 1px solid var(--border-color);
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
        }}
        th, td {{
            padding: 1rem;
            text-align: left;
            border-bottom: 1px solid var(--border-color);
        }}
        th {{
            background-color: var(--bg-tertiary);
            font-weight: 600;
            text-transform: uppercase;
            font-size: 0.75rem;
            letter-spacing: 0.05em;
        }}
        td {{
            color: var(--text-secondary);
        }}
        tr:last-child td {{
            border-bottom: none;
        }}
        .footer {{
            text-align: center;
            margin-top: 3rem;
            padding-top: 2rem;
            border-top: 1px solid var(--border-color);
            color: var(--text-secondary);
            font-size: 0.875rem;
        }}
        @media (max-width: 768px) {{
            .stats-grid {{
                grid-template-columns: repeat(2, 1fr);
            }}
        }}
        @media print {{
            body {{
                background-color: white;
                color: black;
            }}
            .stat-card, .card {{
                border: 1px solid #ddd;
                background-color: #f9f9f9;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>🦀 Chaos Test Report</h1>
            <p class="subtitle">{scenario_name}</p>
        </header>

        <div class="stats-grid">
            <div class="stat-card">
                <div class="stat-icon">⏱️</div>
                <div class="stat-value">{duration_secs}s</div>
                <div class="stat-label">Duration</div>
            </div>
            <div class="stat-card">
                <div class="stat-icon">📊</div>
                <div class="stat-value {success_class}">{success_rate:.1}%</div>
                <div class="stat-label">Success Rate</div>
            </div>
            <div class="stat-card">
                <div class="stat-icon">⚡</div>
                <div class="stat-value">{injections}</div>
                <div class="stat-label">Injections</div>
            </div>
            <div class="stat-card">
                <div class="stat-icon">📋</div>
                <div class="stat-value">{phases}</div>
                <div class="stat-label">Phases</div>
            </div>
        </div>

        <div class="card">
            <h2 class="card-title">Phase Results</h2>
            <table>
                <thead>
                    <tr>
                        <th>Phase</th>
                        <th>Duration</th>
                        <th>Injections</th>
                    </tr>
                </thead>
                <tbody>
                    {phases_html}
                </tbody>
            </table>
        </div>

        <div class="card">
            <h2 class="card-title">SLO Gate: <span class="stat-value {slo_class}">{slo_gate}</span></h2>
            <table>
                <thead>
                    <tr>
                        <th>Assertion</th>
                        <th>Result</th>
                        <th>Probes</th>
                        <th>Error Rate</th>
                        <th>P95 Latency</th>
                    </tr>
                </thead>
                <tbody>
                    {slo_results_html}
                </tbody>
            </table>
        </div>

        <footer class="footer">
            <p>Generated by Chaos Engineering Framework • {timestamp}</p>
        </footer>
    </div>
</body>
</html>"##,
        scenario_name = result.scenario_name,
        duration_secs = result.total_duration.as_secs(),
        success_class = success_class,
        success_rate = success_rate,
        injections = result.total_injections,
        phases = result.phase_results.len(),
        phases_html = phases_html,
        slo_class = slo_class,
        slo_gate = slo_gate,
        slo_results_html = slo_results_html,
        timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario_result() -> chaos_scenarios::runner::ScenarioResult {
        use chaos_scenarios::runner::{PhaseResult, ScenarioResult};
        ScenarioResult {
            scenario_name: "report test".to_string(),
            started_at: chrono::Utc::now(),
            total_duration: std::time::Duration::from_secs(2),
            phase_results: vec![PhaseResult {
                name: "steady state".to_string(),
                duration: std::time::Duration::from_secs(2),
                injection_count: 1,
                attempted_injections: 1,
                injection_failures: vec![],
                cleanup_failures: vec![],
            }],
            total_injections: 1,
            attempted_injections: 1,
            cleanup_failures: 0,
            slo_results: vec![],
            telemetry: chaos_scenarios::runner::RunTelemetrySnapshot::default(),
            cancelled: false,
        }
    }

    #[test]
    fn generates_markdown_and_html_reports() {
        let result = scenario_result();
        let markdown = generate_markdown_report(&result);
        let html = generate_html_report(&result);

        assert!(markdown.contains("report test"));
        assert!(markdown.contains("steady state"));
        assert!(html.contains("report test"));
        assert!(html.contains("steady state"));
    }

    #[test]
    fn compares_baseline_metrics() {
        let baseline = scenario_result();
        let mut chaos = scenario_result();
        chaos.total_duration = std::time::Duration::from_secs(4);
        assert_eq!(
            duration_ratio(chaos.total_duration, baseline.total_duration),
            2.0
        );
        assert_eq!(combined_slo_error_rate(&chaos), 0.0);
    }
}

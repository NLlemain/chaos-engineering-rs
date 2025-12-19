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

    if !compare.is_empty() {
        println!("\n{}", "Comparison mode not yet implemented".yellow());
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
}

fn generate_markdown_report(result: &chaos_scenarios::runner::ScenarioResult) -> String {
    format!(
        r#"# Chaos Test Report: {}

## Summary

- **Total Duration**: {:?}
- **Total Injections**: {}
- **Success Rate**: {:.2}%

## Phase Results

{}

## Conclusion

Test completed successfully.
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
            .join("\n")
    )
}

fn generate_html_report(result: &chaos_scenarios::runner::ScenarioResult) -> String {
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
        timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )
}

use crate::config::{Scenario, ScenarioConfig};
use anyhow::Result;
use std::path::Path;

pub async fn parse_scenario_from_file(path: impl AsRef<Path>) -> Result<Scenario> {
    let path = path.as_ref();
    let contents = tokio::fs::read_to_string(path).await?;

    let extension = path.extension().and_then(|s| s.to_str());

    match extension {
        Some("yaml") | Some("yml") => parse_yaml(&contents),
        Some("toml") => parse_toml(&contents),
        Some("json") => parse_json(&contents),
        _ => Err(anyhow::anyhow!(
            "Unsupported file format. Use .yaml, .yml, .toml, or .json"
        )),
    }
}

pub fn parse_scenario_from_str(content: &str, format: &str) -> Result<Scenario> {
    match format.to_lowercase().as_str() {
        "yaml" | "yml" => parse_yaml(content),
        "toml" => parse_toml(content),
        "json" => parse_json(content),
        _ => Err(anyhow::anyhow!("Unsupported format: {}", format)),
    }
}

fn parse_yaml(content: &str) -> Result<Scenario> {
    let scenario: Scenario = serde_yaml::from_str(content)?;
    scenario.validate().map_err(|e| anyhow::anyhow!(e))?;
    Ok(scenario)
}

fn parse_toml(content: &str) -> Result<Scenario> {
    let config: ScenarioConfig = toml::from_str(content)?;
    config.scenario.validate().map_err(|e| anyhow::anyhow!(e))?;
    Ok(config.scenario)
}

fn parse_json(content: &str) -> Result<Scenario> {
    let scenario: Scenario = serde_json::from_str(content)?;
    scenario.validate().map_err(|e| anyhow::anyhow!(e))?;
    Ok(scenario)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
name: "test_scenario"
duration: 120s
phases:
  - name: "phase1"
    duration: 60s
    injections: []
"#;

        let scenario = parse_yaml(yaml).unwrap();
        assert_eq!(scenario.name, "test_scenario");
        assert_eq!(scenario.phases.len(), 1);
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
[scenario]
name = "test_scenario"
duration = "120s"

[[scenario.phases]]
name = "phase1"
duration = "60s"
injections = []
"#;

        let scenario = parse_toml(toml).unwrap();
        assert_eq!(scenario.name, "test_scenario");
    }

    #[test]
    fn test_parse_json() {
        let json = r#"
{
  "name": "test_scenario",
  "duration": "120s",
  "phases": [
    {
      "name": "phase1",
      "duration": "60s",
      "injections": []
    }
  ]
}
"#;

        let scenario = parse_json(json).unwrap();
        assert_eq!(scenario.name, "test_scenario");
    }
}

use crate::aggregator::AggregatedMetrics;
use anyhow::Result;
use std::path::Path;

pub struct JsonExporter;

impl JsonExporter {
    pub async fn export(metrics: &AggregatedMetrics, path: impl AsRef<Path>) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    pub fn to_string(metrics: &AggregatedMetrics) -> Result<String> {
        Ok(serde_json::to_string_pretty(metrics)?)
    }
}

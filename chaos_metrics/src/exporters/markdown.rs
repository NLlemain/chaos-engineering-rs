use crate::aggregator::AggregatedMetrics;
use anyhow::Result;
use std::path::Path;

pub struct MarkdownExporter;

impl MarkdownExporter {
    pub async fn export(metrics: &AggregatedMetrics, path: impl AsRef<Path>) -> Result<()> {
        let markdown = Self::format(metrics);
        tokio::fs::write(path, markdown).await?;
        Ok(())
    }

    pub fn format(metrics: &AggregatedMetrics) -> String {
        format!(
            r#"# Chaos Engineering Test Report

## Summary Statistics

| Metric | Value |
|--------|-------|
| Total Requests | {} |
| Successful Requests | {} |
| Failed Requests | {} |
| Error Rate | {:.2}% |

## Latency Distribution

| Percentile | Latency |
|------------|---------|
| P50 | {:?} |
| P95 | {:?} |
| P99 | {:?} |
| P99.9 | {:?} |
| Average | {:?} |
| Min | {:?} |
| Max | {:?} |

## Recovery Metrics

| Metric | Value |
|--------|-------|
| Average Recovery Time | {:?} |

## Conclusion

Test completed. Review the metrics above to assess system resilience.
"#,
            metrics.total_requests,
            metrics.successful_requests,
            metrics.failed_requests,
            metrics.error_rate * 100.0,
            metrics.latency_p50,
            metrics.latency_p95,
            metrics.latency_p99,
            metrics.latency_p999,
            metrics.average_latency,
            metrics.min_latency,
            metrics.max_latency,
            metrics.average_recovery_time,
        )
    }
}

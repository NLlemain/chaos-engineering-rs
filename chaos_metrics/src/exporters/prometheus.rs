use crate::aggregator::AggregatedMetrics;

pub struct PrometheusExporter;

impl PrometheusExporter {
    pub fn format(metrics: &AggregatedMetrics) -> String {
        format!(
            r#"# HELP chaos_total_requests Total number of requests
# TYPE chaos_total_requests counter
chaos_total_requests {}

# HELP chaos_failed_requests Total number of failed requests
# TYPE chaos_failed_requests counter
chaos_failed_requests {}

# HELP chaos_error_rate Error rate
# TYPE chaos_error_rate gauge
chaos_error_rate {}

# HELP chaos_latency_p50 50th percentile latency in seconds
# TYPE chaos_latency_p50 gauge
chaos_latency_p50 {}

# HELP chaos_latency_p95 95th percentile latency in seconds
# TYPE chaos_latency_p95 gauge
chaos_latency_p95 {}

# HELP chaos_latency_p99 99th percentile latency in seconds
# TYPE chaos_latency_p99 gauge
chaos_latency_p99 {}

# HELP chaos_avg_latency Average latency in seconds
# TYPE chaos_avg_latency gauge
chaos_avg_latency {}
"#,
            metrics.total_requests,
            metrics.failed_requests,
            metrics.error_rate,
            metrics.latency_p50.as_secs_f64(),
            metrics.latency_p95.as_secs_f64(),
            metrics.latency_p99.as_secs_f64(),
            metrics.average_latency.as_secs_f64(),
        )
    }
}

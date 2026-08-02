use crate::aggregator::AggregatedMetrics;
use chaos_scenarios::runner::RunTelemetrySnapshot;

pub struct PrometheusExporter;

impl PrometheusExporter {
    pub fn format_run(metrics: &RunTelemetrySnapshot) -> String {
        let probe_error_rate = if metrics.probes_total == 0 {
            0.0
        } else {
            metrics.probes_failed as f64 / metrics.probes_total as f64
        };
        format!(
            "# HELP chaos_run_active Whether a chaos scenario is currently running\n\
# TYPE chaos_run_active gauge\n\
chaos_run_active {}\n\
# HELP chaos_injections_attempted_total Attempted fault injections\n\
# TYPE chaos_injections_attempted_total counter\n\
chaos_injections_attempted_total {}\n\
# HELP chaos_injections_succeeded_total Successful fault injections\n\
# TYPE chaos_injections_succeeded_total counter\n\
chaos_injections_succeeded_total {}\n\
# HELP chaos_cleanup_failures_total Failed fault cleanup operations\n\
# TYPE chaos_cleanup_failures_total counter\n\
chaos_cleanup_failures_total {}\n\
# HELP chaos_slo_probes_total HTTP SLO probes completed\n\
# TYPE chaos_slo_probes_total counter\n\
chaos_slo_probes_total {}\n\
# HELP chaos_slo_probe_failures_total Failed HTTP SLO probes\n\
# TYPE chaos_slo_probe_failures_total counter\n\
chaos_slo_probe_failures_total {}\n\
# HELP chaos_slo_probe_error_rate Current SLO probe error rate\n\
# TYPE chaos_slo_probe_error_rate gauge\n\
chaos_slo_probe_error_rate {}\n",
            u8::from(metrics.active),
            metrics.injections_attempted,
            metrics.injections_succeeded,
            metrics.cleanup_failures,
            metrics.probes_total,
            metrics.probes_failed,
            probe_error_rate,
        )
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_run_format_has_prometheus_types_and_final_newline() {
        let output = PrometheusExporter::format_run(&RunTelemetrySnapshot {
            active: true,
            injections_attempted: 3,
            injections_succeeded: 2,
            cleanup_failures: 1,
            probes_total: 4,
            probes_failed: 1,
        });
        assert!(output.contains("# TYPE chaos_run_active gauge"));
        assert!(output.contains("chaos_slo_probe_error_rate 0.25"));
        assert!(output.ends_with('\n'));
    }
}

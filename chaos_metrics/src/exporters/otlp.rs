use anyhow::{bail, Context, Result};
use chaos_scenarios::runner::RunTelemetrySnapshot;
use serde_json::{json, Value};

pub struct OtlpHttpExporter;

impl OtlpHttpExporter {
    pub async fn export(
        endpoint: &str,
        service_name: &str,
        metrics: &RunTelemetrySnapshot,
    ) -> Result<()> {
        let endpoint = metrics_endpoint(endpoint)?;
        let payload = Self::payload(service_name, metrics);
        let response = reqwest::Client::new()
            .post(endpoint.clone())
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("Failed to export OTLP metrics to {}", endpoint))?;
        if !response.status().is_success() {
            bail!(
                "OTLP collector {} returned HTTP {}",
                endpoint,
                response.status()
            );
        }
        Ok(())
    }

    pub fn payload(service_name: &str, metrics: &RunTelemetrySnapshot) -> Value {
        let timestamp = chrono::Utc::now()
            .timestamp_nanos_opt()
            .unwrap_or_default()
            .to_string();
        let probe_error_rate = if metrics.probes_total == 0 {
            0.0
        } else {
            metrics.probes_failed as f64 / metrics.probes_total as f64
        };
        let counters = [
            ("chaos.injections.attempted", metrics.injections_attempted),
            ("chaos.injections.succeeded", metrics.injections_succeeded),
            ("chaos.cleanup.failures", metrics.cleanup_failures),
            ("chaos.slo.probes", metrics.probes_total),
            ("chaos.slo.probe_failures", metrics.probes_failed),
        ]
        .into_iter()
        .map(|(name, value)| {
            json!({
                "name": name,
                "sum": {
                    "aggregationTemporality": 2,
                    "isMonotonic": true,
                    "dataPoints": [{"timeUnixNano": timestamp, "asInt": value.to_string()}]
                }
            })
        });
        let mut metric_values: Vec<_> = counters.collect();
        metric_values.push(json!({
            "name": "chaos.run.active",
            "gauge": {"dataPoints": [{
                "timeUnixNano": timestamp,
                "asDouble": if metrics.active { 1.0 } else { 0.0 }
            }]}
        }));
        metric_values.push(json!({
            "name": "chaos.slo.probe_error_rate",
            "gauge": {"dataPoints": [{
                "timeUnixNano": timestamp,
                "asDouble": probe_error_rate
            }]}
        }));

        json!({
            "resourceMetrics": [{
                "resource": {"attributes": [{
                    "key": "service.name",
                    "value": {"stringValue": service_name}
                }]},
                "scopeMetrics": [{
                    "scope": {"name": "chaos-engineering-rs"},
                    "metrics": metric_values
                }]
            }]
        })
    }
}

fn metrics_endpoint(endpoint: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(endpoint).context("Invalid OTLP endpoint")?;
    if url.path().is_empty() || url.path() == "/" {
        url.set_path("/v1/metrics");
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn creates_otlp_json_metrics_request() {
        let payload = OtlpHttpExporter::payload(
            "checkout-chaos",
            &RunTelemetrySnapshot {
                active: false,
                injections_attempted: 2,
                injections_succeeded: 2,
                cleanup_failures: 0,
                probes_total: 10,
                probes_failed: 1,
            },
        );
        assert_eq!(
            payload["resourceMetrics"][0]["resource"]["attributes"][0]["value"]["stringValue"],
            "checkout-chaos"
        );
        assert_eq!(
            metrics_endpoint("http://localhost:4318").unwrap().path(),
            "/v1/metrics"
        );
    }

    #[tokio::test]
    async fn posts_metrics_to_a_real_otlp_http_endpoint() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let capture = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let count = stream.read(&mut buffer).await.unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if complete_http_request(&request) {
                    break;
                }
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}")
                .await
                .unwrap();
            String::from_utf8(request).unwrap()
        });

        OtlpHttpExporter::export(
            &format!("http://{}", address),
            "integration-chaos",
            &RunTelemetrySnapshot::default(),
        )
        .await
        .unwrap();
        let request = capture.await.unwrap();
        assert!(request.starts_with("POST /v1/metrics HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("content-type: application/json"));
        assert!(request.contains("integration-chaos"));
    }

    fn complete_http_request(request: &[u8]) -> bool {
        let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        });
        content_length.is_some_and(|length| request.len() >= header_end + 4 + length)
    }
}

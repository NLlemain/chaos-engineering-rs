# Scenario Schema and Invariants

Guide for authoring custom chaos scenarios with phase structures, injector configurations, and SLO assertions.

## Scenario File Structure

A scenario is defined as a YAML file with the following top-level fields:

```yaml
name: api_resilience_gate
description: Injects dependency latency and verifies HTTP API SLO thresholds.
duration: 30s
labels:
  environment: staging
  tier: core-api

phases:
  - name: baseline_observation
    duration: 5s
    injections: []

  - name: degraded_downstream
    duration: 20s
    injections:
      - type: dependency_proxy
        target:
          address: 127.0.0.1:5432
        listen: 127.0.0.1:15432
        direction: downstream
        latency: 200ms
        jitter: 30ms

  - name: recovery_verification
    duration: 5s
    injections: []

assertions:
  - name: health_probe_availability
    url: http://127.0.0.1:8080/health
    expected_status: 200
    interval: 500ms
    timeout: 1s
    max_error_rate: 0.02
    max_p95_latency: 300ms
    min_requests: 20
```

---

## Phases and Injections

Each phase executes for its configured `duration`. Injections active during that phase are automatically initialized and torn down at the phase boundary.

### Supported Injector Types

| Injector Type | Target Description | Key Parameters |
|---|---|---|
| `dependency_proxy` | TCP downstream / upstream | `target.address`, `listen`, `direction`, `latency`, `jitter`, `bandwidth_bps` |
| `http_fault` | HTTP service | `listen`, `upstream`, `status_code`, `delay`, `drop_rate` |
| `container_fault` | Docker / Compose service | `container_id`, `action` (`pause`, `restart`), `duration` |
| `database_fault` | SQLite / DuckDB | `engine`, `path`, `mode` (`read_only`, `unavailable`) |
| `memory_pressure` | System memory | `bytes`, `duration` |
| `cpu_starvation` | CPU cores | `cores`, `load_percentage`, `duration` |
| `dns_fault` | DNS queries | `domain`, `spoof_ip`, `latency` |

---

## SLO Assertions

Assertions continuously probe one or more endpoints throughout the experiment:

- `name`: Unique identifier for the assertion in metrics and reports.
- `url`: HTTP / HTTPS probe URL.
- `expected_status`: Expected HTTP response code (e.g. `200`).
- `interval`: Time between probe requests (e.g. `250ms`, `1s`).
- `timeout`: Per-request connection timeout (e.g. `500ms`).
- `max_error_rate`: Maximum allowed error rate fraction (e.g. `0.05` for 5%).
- `max_p95_latency`: Maximum allowed 95th percentile response latency (e.g. `200ms`).
- `min_requests`: Minimum number of probe samples required before evaluating the assertion.

If any assertion fails during `chaos run`, the process exits with a non-zero status code for CI/CD gates.

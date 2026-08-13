# Chaos Engineering RS

[![CI](https://github.com/Ninian-Lemain/chaos-engineering-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Ninian-Lemain/chaos-engineering-rs/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Ninian-Lemain/chaos-engineering-rs)](https://github.com/Ninian-Lemain/chaos-engineering-rs/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE-MIT)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-555)](#capability-matrix)

**Evidence-driven chaos engineering for applications, infrastructure, and real-time data.**

Chaos Engineering RS is a cross-platform Rust CLI and library for fault injection, resilience testing, and recovery verification. Test APIs, Docker and Kubernetes workloads, databases, networks, AI streams, data pipelines, observability, media, IoT, storage, distributed systems, and trading infrastructure. Experiments are reproducible, policy bounded, measurable, and recoverable rather than a list of simulated failure names.

![Chaos dashboard](docs/assets/dashboard.png)

## What Makes It Different

- **Distributed control:** mTLS agents use prepare/execute/recover commands, deterministic per-target seeds, coordinated phases, and policy-bounded parallelism.
- **Market-microstructure evidence:** replay market-data gaps, duplicates, reordering, clock skew, venue partitions, and order-ack latency; mutate FIX sequence, `PossDup`, reject, and checksum behavior.
- **Zero-buffer pipeline evidence:** rendezvous producers directly with consumers, measure real backpressure, and replay loss, truncation, event-time regressions, hot keys, schema poison, CDC boundaries, and telemetry cardinality faults.
- **Recovery first:** every recoverable effect is journaled, Kubernetes changes retain their original state, and interrupted experiments can be stopped centrally.
- **Honest effects:** an injector cannot report success unless it produced a measurable disruption; stable packs require CI evidence for disruption and restoration.
- **Rootless and CI ready:** dependency, DNS, TLS, HTTP, AI, database, queue, media, storage, and trading faults can run through local proxies with SLO exit gates.

![Thirty-second terminal tour](docs/assets/terminal-demo.gif)

## Quick Start

Install the current release from [GitHub Releases](https://github.com/Ninian-Lemain/chaos-engineering-rs/releases), or build from source:

```bash
git clone https://github.com/Ninian-Lemain/chaos-engineering-rs
cd chaos-engineering-rs
cargo build --locked --release -p chaos_cli
./target/release/chaos doctor
./target/release/chaos list
```

Homebrew is available now:

```bash
brew install Ninian-Lemain/chaos-engineering/chaos-engineering-rs
```

The [WinGet submission](https://github.com/microsoft/winget-pkgs/pull/411247) is awaiting Microsoft review. After merge, install it with `winget install NinianLemain.ChaosEngineeringRs`.

On Windows, run `target\release\chaos.exe`. Before an experiment:

```bash
chaos validate scenarios/slo_gate.yaml
chaos dry-run scenarios/slo_gate.yaml
chaos run scenarios/slo_gate.yaml --output-json result.json
```

Open the local control surface:

```bash
chaos serve --host 127.0.0.1 --port 8080
```

See [QUICKSTART.md](QUICKSTART.md) for proxy, Docker, database, telemetry, report, and recovery examples.

## Zero-Buffer Data Pipelines

`chaos pipeline replay` tests generic JSON Lines streams with a capacity-zero rendezvous channel. Every producer send waits for a ready consumer; there is no hidden queue between stages. Reports measure producer-blocking p50/p99/max, record loss, sequence gaps, duplicates, ordering, timestamp regressions, content digests, and exact restored delivery.

```bash
chaos pipeline replay tests/pipeline-evidence/records.jsonl \
  --fault-plan tests/pipeline-evidence/zero-buffer-stall.yaml \
  --budget tests/pipeline-evidence/budget.yaml \
  --output pipeline-evidence.json

chaos pack list --category data-pipelines
chaos pack list --search backpressure
```

The same record model can represent WebSocket feeds, CDC/outbox events, queue consumers, ETL/ELT stages, telemetry, crypto order books, and in-process channels. Faults include consumer stalls, end-of-stream truncation, drops, duplication, reordering, partition outages, timestamp regression, JSON Pointer corruption, commit-marker loss, sequence resets, cardinality explosion, and routing-key collapse.

## HFT And Quant Engineering

The HFT workflow is deterministic and offline by default. A JSON Lines event stream is replayed three ways: baseline, faulted, and restored. The command fails unless the baseline satisfies its invariant budget, chaos has a measurable effect, and restored state exactly matches the baseline digest.

```bash
chaos hft replay tests/hft-evidence/market-events.jsonl \
  --fault-plan tests/hft-evidence/sequence-gap.yaml \
  --budget tests/hft-evidence/invariants.yaml \
  --output hft-evidence.json

chaos hft fix tests/hft-evidence/orders.fix \
  --fault-plan tests/hft-evidence/fix-faults.yaml \
  --output faulted-orders.fix
```

Evidence covers per-venue sequence gaps, duplicate and out-of-order messages, stale timestamps, crossed books, rejected orders, p50/p99 acknowledgement latency, deterministic state hashes, FIX gaps, duplicates, `PossDup`, execution rejects, and checksum corruption. Download network experiments with `chaos pack list --category trading`.

## Distributed Experiments

Agents reject clients without a certificate signed by the configured CA. Both the orchestrator and each agent enforce injector, target, schedule, duration, SLO, target-count, parallelism, and blast-radius policy before a scenario can run.

```bash
chaos agent serve --id ams-1 --listen 0.0.0.0:9443 \
  --ca-cert certs/ca.pem --cert certs/ams-1.pem --key certs/ams-1-key.pem \
  --policy examples/distributed-policy.yaml

chaos distributed examples/distributed-experiment.yaml \
  --ca-cert certs/ca.pem --cert certs/orchestrator.pem \
  --key certs/orchestrator-key.pem --policy examples/distributed-policy.yaml

chaos history list
chaos history show EXPERIMENT_ID
chaos history prune --max-runs 500 --max-age-days 30
```

Central SQLite history stores the exact root seed, per-target results, manifest and policy SHA-256 digests, and content-addressed artifacts. See [Distributed Experiments](docs/DISTRIBUTED.md) for certificate and manifest setup.

## Kubernetes Faults

`kubernetes_fault` is experimental and uses the current `kubectl` identity. Before mutation it confirms RBAC for every required operation. Network isolation deterministically labels only the policy-bounded pod subset and applies an empty ingress/egress `NetworkPolicy`; cleanup removes both. Deployment and StatefulSet scale faults record and restore the original replica count.

```bash
chaos dry-run examples/kubernetes-network-isolation.yaml
chaos run examples/kubernetes-network-isolation.yaml
```

## Rootless Faults

`chaos proxy` provides directional latency, jitter, bandwidth limits, connection timeouts, slow closes, byte limits, partitions, corruption, duplication, reordering, and connection-pool pressure. `chaos dns-proxy`, `chaos tls-endpoint`, and `chaos ai-proxy` add DNS answers, TLS handshake failures, HTTP delay/status/body/header faults, delayed tokens, broken SSE streams, malformed tool calls, 429 storms, and context truncation. Downloadable profiles cover WebSocket slow consumers, gRPC stream truncation, Redis/Valkey response lag, and partial GraphQL responses.

```bash
chaos proxy --listen 127.0.0.1:15432 --upstream 127.0.0.1:5432 \
  --direction downstream --latency 250ms --bandwidth 65536

chaos ai-proxy --provider open-ai --listen 127.0.0.1:18080 \
  --upstream https://api.openai.com --stream-delay 400ms
```

Point the application at the local listener; no kernel network rules are required.

## Scenario Packs

The catalog covers AI APIs, authentication, containers, data pipelines, databases, IoT/MQTT, media/HLS/WebRTC, object storage, observability/OpenTelemetry, queues, trading/FIX/crypto market data, network/DNS/gRPC/GraphQL/WebSocket, and Windows. Search and install only what a test needs:

```bash
chaos pack list --category ai
chaos pack list --category data-pipelines
chaos pack list --search opentelemetry
chaos pack list --search crypto
chaos pack show ai-openai-compatible
chaos pack install ai-openai-compatible --output ./scenarios
```

Pack status is tracked independently in [`scenario-packs/catalog.json`](scenario-packs/catalog.json). See [`scenario-packs/README.md`](scenario-packs/README.md) to author a pack.

## SLO Gates And Reports

Scenarios can continuously probe an endpoint and enforce error-rate, p95-latency, status, and minimum-sample requirements. A failed assertion exits nonzero for CI.

```yaml
assertions:
  - name: api_availability
    url: http://127.0.0.1:8080/health
    expected_status: 200
    interval: 500ms
    timeout: 1s
    max_error_rate: 0.05
    max_p95_latency: 250ms
    min_requests: 10
```

```bash
chaos run scenario.yaml --output-json chaos.json --prometheus-port 9898 \
  --otlp-endpoint http://127.0.0.1:4318/v1/metrics
chaos report chaos.json --compare baseline.json --format markdown --output comparison.md
```

Use the repository action from another workflow:

```yaml
- uses: Ninian-Lemain/chaos-engineering-rs@v0.4.0
  with:
    scenario: scenarios/api-slo.yaml
    output: chaos-result.json
```

## Safety And Recovery

Chaos tests are destructive by design. Start with disposable targets and narrow permissions.

```bash
chaos doctor                    # dependencies, permissions, journal state
chaos doctor --json             # machine-readable checks for CI and editors
chaos dry-run scenario.yaml     # validation without faults
chaos recover                   # restore interrupted effects
chaos stop-all                  # emergency cleanup of all journaled effects
```

The default journal is `~/.chaos-engineering/recovery.json`. `doctor` reports missing commands, daemon access, elevation, and blocked injectors before a run.

## Capability Matrix

Injector and pack status is limited to **stable**, **experimental**, or **planned**. `chaos list` is the source of truth for operating-system injectors, while `chaos pack list` covers downloadable protocol scenarios and offline fault plans. `chaos list --json` exposes the 25-entry runtime registry to scripts, including each injector's required capabilities. `chaos doctor` adds permission and dependency checks.

<!-- BEGIN GENERATED CAPABILITY MATRIX -->
| Injector | Status | Required capabilities |
|---|---|---|
| `aws_fault` | planned | None |
| `azure_fault` | planned | None |
| `clock_skew` | planned | CAP_SYS_TIME |
| `cloudflare_fault` | planned | None |
| `container_fault` | stable | Docker CLI and daemon access |
| `cpu_starvation` | stable | None |
| `crypto_fault` | stable | None |
| `database_fault` | stable | Read/write access to the database directory |
| `dependency_proxy` | stable | None |
| `disk_fill` | stable | Write access to the target directory |
| `disk_slow` | planned | None |
| `dns_fault` | stable | None |
| `fd_exhaustion` | stable | None |
| `http_fault` | stable | None |
| `kubernetes_fault` | experimental | kubectl, Kubernetes API access, RBAC for pods and network policies or workload scaling |
| `media_streaming_fault` | planned | None |
| `memory_pressure` | stable | None |
| `network_latency` | experimental | CAP_NET_ADMIN |
| `nginx_fault` | planned | None |
| `packet_loss` | experimental | CAP_NET_ADMIN |
| `process_freeze` | stable | Permission to signal the target process |
| `process_kill` | experimental | Permission to signal the target process |
| `socket_corrupt` | planned | CAP_NET_RAW, CAP_NET_ADMIN |
| `tcp_reset` | experimental | CAP_NET_ADMIN |
| `windows_fault` | planned | None |
<!-- END GENERATED CAPABILITY MATRIX -->

Integration tests verify that stable effects disrupt their target and restore recoverable state. Planned injectors fail closed instead of pretending to run.

## Distribution

| Channel | State |
|---|---|
| GitHub binaries | Release workflow builds Windows x64, Linux x64, macOS x64/arm64 with SHA-256 files and GitHub attestations |
| GitHub Action | `action.yml` runs a scenario as an SLO gate on Windows, Linux, and macOS |
| Container | GHCR workflow builds amd64/arm64 images |
| Homebrew | Live at [`Ninian-Lemain/homebrew-chaos-engineering`](https://github.com/Ninian-Lemain/homebrew-chaos-engineering) |
| WinGet | [Validated manifest submitted](https://github.com/microsoft/winget-pkgs/pull/411247); awaiting Microsoft review |
| crates.io | Publish workflow is ready; the first release requires a personal `CARGO_REGISTRY_TOKEN` |

Verify a downloaded release artifact with GitHub CLI:

```bash
gh attestation verify chaos-v0.4.0-x86_64-unknown-linux-gnu.tar.gz \
  --repo Ninian-Lemain/chaos-engineering-rs
```

## Project

- [Roadmap](docs/ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Changelog](CHANGES.md)
- [Discussions](https://github.com/Ninian-Lemain/chaos-engineering-rs/discussions)
- [Issues](https://github.com/Ninian-Lemain/chaos-engineering-rs/issues)

Licensed under the [MIT License](LICENSE-MIT).

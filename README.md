# 🦀 Chaos Engineering Framework

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/Ninian-Lemain/chaos-engineering-rs)
[![CI](https://github.com/Ninian-Lemain/chaos-engineering-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/Ninian-Lemain/chaos-engineering-rs/actions/workflows/ci.yml)

**A lightweight, cross-platform chaos engineering framework for testing service resilience through controlled failure injection.**

Built in Rust for performance and safety. Test how your services handle real-world failures like network issues, resource exhaustion, multi-cloud outages, edge CDN drops, media streaming disruptions, and process crashes.

## ✨ Features

- **🌐 Cross-Platform**: Windows, macOS, Linux with platform-native chaos injection and graceful cross-platform fallback
- **⚡ High Performance**: Async Rust, ~15MB memory, <1% CPU overhead
- **🎯 Honest Capability Registry**: 23 discoverable chaos types with stable, experimental, and planned status shown directly by `chaos list`
- **📋 YAML Configuration**: Declarative test scenarios with multi-phase support
- **🖥️ Web Dashboard**: Dark-themed UI for test management and monitoring
- **🔥 Load Testing**: Rate-limited concurrent load tests for HTTP/HTTPS APIs and HLS manifests
- **📊 Multiple Outputs**: CLI, JSON, Markdown, and HTML reports
- **🛡️ Guardrails**: Configuration validation, explicit destructive-operation warnings, and clear privilege separation

## 🚀 Quick Start

### Prerequisites

- **Latest stable Rust** - [Install Rust](https://www.rust-lang.org/tools/install)
- **Windows**: Native support with OS level process & resource handles
- **Linux**: `iproute2`, `iptables` (usually pre-installed)
- **macOS**: Built-in tools, requires sudo for network chaos

### Installation

```bash
git clone https://github.com/Ninian-Lemain/chaos-engineering-rs
cd chaos-engineering-rs
cargo build --release
```

### Run Your First Test

```bash
# Start test service
./target/release/axum_http_service

# Run chaos test (new terminal)
./target/release/chaos run scenarios/quick_test.yaml --verbose
```

### Launch Web Dashboard

```bash
./target/release/chaos serve --port 8080
# Open http://127.0.0.1:8080
```

### Start a Rootless Dependency Proxy

Route an application through the printed local address to inject real faults without administrator privileges:

```bash
chaos proxy \
  --upstream 127.0.0.1:5432 \
  --listen 127.0.0.1:15432 \
  --direction downstream \
  --latency 250ms \
  --jitter 40ms \
  --bandwidth 65536
```

The proxy supports latency, bandwidth limits, timeouts, slow closes, byte limits, partitions, corruption, duplication, reordering, and per-connection toxicity. Use `scenarios/dependency_proxy.yaml` for the equivalent declarative configuration.

### Test AI APIs and Streaming Clients

The provider-aware HTTP proxy supports OpenAI, Azure OpenAI, Anthropic, Gemini, OpenRouter, Ollama, Mistral, Groq, Cohere, Together, vLLM, and generic APIs:

```bash
chaos ai-proxy \
  --provider anthropic \
  --upstream https://api.anthropic.com \
  --listen 127.0.0.1:18082 \
  --stream-delay 120ms \
  --stream-abort 4 \
  --malformed-tool-call
```

Point the SDK base URL at the printed local address. The proxy forwards existing authentication headers while injecting provider-shaped 429/5xx errors, delayed SSE or NDJSON events, broken streams, context truncation, malformed tool calls, invalid JSON, empty responses, truncated bodies, and semantic header corruption. Ready-to-edit profiles live in `scenario-packs/ai/`.

### Test DNS and TLS Without Administrator Rights

```bash
chaos dns-proxy --mode spoof --domain "*.internal" --fake-ip 127.0.0.42
chaos tls-endpoint --mode incomplete-chain --domain localhost
```

Point the resolver or TLS client at the printed local endpoint. DNS supports forwarding, selective delay, NXDOMAIN, spoofed IPv4/IPv6 answers, and blackholes. TLS supports expired and untrusted certificates, missing intermediates, delayed handshakes, and abrupt handshake closure. The endpoints are journaled and removed by `recover` or `stop-all` after interrupted runs.

### Disrupt Docker and Compose Targets

```bash
chaos container --id my-api --action pause --duration 30s
chaos container --compose-service api --compose-file compose.yaml --action kill --duration 15s
```

Container IDs are resolved with Docker itself; Compose services resolve every current replica. Pause, stop, and kill operations record prior state in the recovery journal and restore it on cleanup. `restart` performs an immediate observable restart and does not invent a persistent fault.

### Test DuckDB, SQLite, PostgreSQL, and MySQL

```bash
chaos database --engine duckdb --file data/app.duckdb --mode unavailable --duration 30s
chaos proxy --upstream 127.0.0.1:5432 --listen 127.0.0.1:15432 --max-connections 2
```

File-backed DuckDB and SQLite faults include byte-preserving unavailability, read-only permissions, advisory locks, real neighboring I/O pressure, and controlled inode pressure. PostgreSQL and MySQL use the protocol-agnostic proxy for disconnects, delayed responses, and connection-pool exhaustion. Ready-to-edit profiles live in `scenario-packs/databases/`.

### Gate CI With SLOs and Export Metrics

```yaml
assertions:
  - name: api_availability
    url: http://127.0.0.1:8080/health
    interval: 500ms
    timeout: 1s
    max_error_rate: 0.05
    max_p95_latency: 250ms
    min_requests: 10
```

```bash
chaos run scenarios/slo_gate.yaml --output-json chaos.json --prometheus-port 9898
chaos run scenarios/slo_gate.yaml --otlp-endpoint http://localhost:4318
chaos report chaos.json --compare baseline.json
```

SLO probes run throughout the scenario. Cleanup and report writes finish before a failed assertion returns a nonzero CI exit code. Prometheus serves live metrics at `/metrics`; OTLP uses HTTP/JSON metrics export and honors `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`. Report comparison shows injection success, duration, probe error-rate, and SLO-gate deltas.

### Search and Download Scenario Packs

Protocol support is shipped as scenario packs so the core stays small and reusable:

```bash
chaos pack list
chaos pack list --category media
chaos pack list --search mqtt
chaos pack show hls-stale-manifest
chaos pack install hls-stale-manifest
```

The machine-readable catalog at `scenario-packs/catalog.json` includes status, protocols, requirements, and download URLs. Current categories cover AI providers, authentication, containers, databases, media streaming, Kafka/RabbitMQ/NATS, MQTT/IoT, DNS, and object storage. Planned capabilities remain searchable but cannot be installed until they produce an honest real-world effect.

## 📦 Chaos Injectors (23 Total)

| Injector | Category | Description | Status |
|----------|----------|-------------|--------|
| `dependency_proxy` | Network | Rootless directional TCP faults for any dependency | Stable |
| `container_fault` | Container | Docker/Compose pause, stop, kill, restart, and restoration | Stable |
| `database_fault` | Storage | DuckDB/SQLite outage, read-only, lock, I/O and inode pressure | Stable/Experimental |
| `network_latency` | Network | Native delay on Linux and Windows | Experimental |
| `packet_loss` | Network | Native Linux packet loss | Experimental (Linux) |
| `tcp_reset` | Network | Native Linux TCP termination | Experimental (Linux) |
| `cpu_starvation` | System | Saturates CPU at specified intensity | Operational |
| `memory_pressure` | System | Allocates memory to target % | Operational |
| `disk_slow` | System | I/O latency injection | Planned |
| `process_kill` | Process | Terminates/restarts processes | Operational |
| `fd_exhaustion` | System | File descriptor leak simulation (`EMFILE`/`ENFILE`) | Operational |
| `process_freeze` | Process | Execution pause or OS suspend | Operational |
| `disk_fill` | Storage | Ballast file allocation to trigger `ENOSPC` | Operational |
| `dns_fault` | Network | Rootless forwarding, delay, NXDOMAIN, spoofing, and blackholes | Stable |
| `clock_skew` | System | Time drift injection for TLS, JWT, and consensus | Planned |
| `socket_corrupt` | Network | Bit-flipping and payload corruption in flight | Planned |
| `http_fault` | L7 Web/AI | Provider errors, delayed or broken streams, body/header/context faults | Stable |
| `nginx_fault` | Reverse Proxy | Upstream resets, 502/504 timeouts, SSL drops | Planned |
| `aws_fault` | Cloud | IMDS blackholing, S3 503, DynamoDB throttle, IAM drop | Planned |
| `azure_fault` | Cloud | ARM 429 throttling, CosmosDB RU exhaustion, Key Vault 403 | Planned |
| `cloudflare_fault` | Edge CDN | Cloudflare 520, 522/524 timeouts, Worker CPU, WAF 403 | Planned |
| `media_streaming_fault` | Media | HLS, DASH, RTSP, and WebRTC disruptions | Planned |
| `crypto_fault` | Security | Expired/untrusted certs, missing chains, handshake delay/abort | Stable* |

`crypto_fault` keeps OCSP, signature corruption, and entropy starvation marked as planned and rejects those modes during validation.

## 🖥️ Web Dashboard

Modern dark-themed web interface for chaos engineering:

- **Dashboard**: Real-time test status, system overview
- **Scenarios**: Browse and run YAML test scenarios
- **Load Testing**: Stress test HTTP/HTTPS APIs and HLS manifests
- **Targets**: Save and manage your test endpoints
- **Results**: View test history with detailed metrics

### Load Testing Your Apps

Go to **Load Test** page and configure:

| Field | Description |
|-------|-------------|
| Target Type | HTTP/HTTPS or HLS |
| URL | Your endpoint (e.g., `http://localhost:3000/api`) |
| Concurrent Users | Parallel connections |
| Requests/Second | Target throughput |
| Duration | Test length in seconds |
| Ramp-up | Gradual load increase time |

**Supported Protocols:**
- **HTTP/HTTPS** - REST APIs, web apps
- **HLS** - HTTP Live Streaming

## 📝 Test Scenarios

```yaml
name: "HTTP & Multi-Cloud Resilience Test"
targets:
  - name: "web_api"
    type: "process"
    process_name: "axum_http_service"

phases:
  - name: "baseline"
    duration: "30s"
    
  - name: "network_and_dns_stress"
    duration: "60s"
    injections:
      - type: "network_latency"
        target: "web_api"
        delay: "100ms"
        jitter: "20ms"
      - type: "dns_fault"
        domain_pattern: "*.api.internal"
        failure_rate: 0.2
  
  - name: "cloud_and_proxy_faults"
    duration: "60s"
    parallel: true
    injections:
      - type: "aws_fault"
        service_fault: "S3SlowDown503"
      - type: "azure_fault"
        service_fault: "CosmosDbRuExhaustion"
      - type: "cloudflare_fault"
        error_code: "Error524TimeoutOccurred"
        
  - name: "recovery"
    duration: "30s"
```

### CLI Commands

```bash
# List injectors
./target/release/chaos list

# Validate scenario
./target/release/chaos validate scenarios/my_test.yaml

# Run test
./target/release/chaos run scenarios/my_test.yaml --verbose

# Run with reports
./target/release/chaos run scenarios/stress_test.yaml \
  --output-json results.json \
  --output-markdown report.md

# Start web dashboard
./target/release/chaos serve --port 8080

# Check dependencies and recovery state
./target/release/chaos doctor

# Validate without applying faults
./target/release/chaos dry-run scenarios/quick_test.yaml

# Clean up after an interrupted run
./target/release/chaos recover

# Emergency cleanup of every journaled injection
./target/release/chaos stop-all
```

## 🧪 Test Services

Three example targets included:

```bash
# HTTP service (port 3000)
./target/release/axum_http_service

# TCP echo server (port 9001)
./target/release/tcp_echo_server

# WebSocket feed (port 9002)
./target/release/websocket_feed
```

## 🏗️ Architecture

```
chaos-engineering-rs/
├── chaos_cli/         CLI and commands
├── chaos_core/        Injection engine (20 fault types)
├── chaos_scenarios/   YAML parser, orchestration
├── chaos_targets/     Target discovery, test services
├── chaos_metrics/     Metrics collection, export
├── chaos_web/         Web dashboard
└── scenarios/         Pre-built test scenarios
```

## 🖥️ Platform Support

| Feature | Linux | macOS | Windows |
|---------|:-----:|:-----:|:-------:|
| CPU/Memory/Disk Chaos | ✅ | ✅ | ✅ |
| Process Control | ✅ | ✅ | ✅ |
| Network Chaos | ✅ tc/netem | ✅ dnctl | ✅ app-level |
| DNS / Socket / L7 Chaos | ✅ | ✅ | ✅ |
| AWS / Azure / Cloudflare Chaos | ✅ | ✅ | ✅ |
| Media & Crypto Chaos | ✅ | ✅ | ✅ |
| Web Dashboard | ✅ | ✅ | ✅ |
| Load Testing | ✅ | ✅ | ✅ |

## ⚡ Performance

| Metric | Value |
|--------|-------|
| Binary Size | ~6 MB |
| Build Time | ~30 seconds |
| Memory | ~15 MB |
| CPU Overhead | <1% |
| Startup | <100ms |

## 🛡️ Safety

- **Input Validation**: All configs validated before execution
- **Explicit Risk Controls**: Destructive operations require opt-in configuration
- **Privilege Separation**: Clear user/root boundaries
- **Audit Logging**: All actions logged with timestamps

### Privilege Requirements

| Operation | Linux/macOS | Windows |
|-----------|-------------|---------|
| Network chaos | `sudo` | User |
| CPU/Memory/Disk | User | User |
| Process kill (own) | User | User |
| Process kill (other) | `sudo` | Admin |

## 📖 Documentation

- [QUICKSTART.md](QUICKSTART.md) - 5-minute setup guide
- [SECURITY.md](SECURITY.md) - Security considerations
- [CHANGES.md](CHANGES.md) - Changelog
- [LICENSE-MIT](LICENSE-MIT) - License

## 🤝 Contributing

1. Fork the repo
2. Create feature branch: `git checkout -b feature/amazing`
3. Make changes with tests
4. Format: `cargo fmt --all`
5. Lint: `cargo clippy --all`
6. Submit PR

## 📜 License

MIT License - See [LICENSE-MIT](LICENSE-MIT)

## 💬 Contact

- **Issues**: [GitHub Issues](https://github.com/NLlemain/chaos-engineering-rs/issues)
- **LinkedIn**: [Ninian Lemain](https://www.linkedin.com/in/ninian-lemain-888524330/)
- **Email**: ninianlmm@gmail.com

---

**Remember:** The goal isn't to break things - it's to learn how systems fail so you can build them better.

*"Everything fails all the time." - Werner Vogels*

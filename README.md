# 🦀 Chaos Engineering Framework

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/NLlemain/chaos-engineering-rs)
[![CI](https://github.com/NLlemain/chaos-engineering-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/NLlemain/chaos-engineering-rs/actions/workflows/ci.yml)

**A lightweight, cross-platform chaos engineering framework for testing service resilience through controlled failure injection.**

Built in Rust for performance and safety. Test how your services handle real-world failures like network issues, resource exhaustion, multi-cloud outages, edge CDN drops, media streaming disruptions, and process crashes.

## ✨ Features

- **🌐 Cross-Platform**: Windows, macOS, Linux with platform-native chaos injection and graceful cross-platform fallback
- **⚡ High Performance**: Async Rust, ~15MB memory, <1% CPU overhead
- **🎯 20 Chaos Types**: Network latency, packet loss, TCP resets, CPU starvation, memory pressure, disk I/O, process kills, DNS faults, FD exhaustion, process freeze, clock skew, disk space fill, socket payload corruption, HTTP/L7 faults, Nginx proxy faults, AWS cloud faults, Azure cloud faults, Cloudflare edge faults, media streaming faults, crypto/security faults
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
git clone https://github.com/NLlemain/chaos-engineering-rs
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

## 📦 Chaos Injectors (20 Total)

| Injector | Category | Description | Platform |
|----------|----------|-------------|----------|
| `network_latency` | Network | Adds delay to packets (mean + jitter) | All |
| `packet_loss` | Network | Randomly drops network packets | All |
| `tcp_reset` | Network | Terminates TCP connections | All |
| `cpu_starvation` | System | Saturates CPU at specified intensity | All |
| `memory_pressure` | System | Allocates memory to target % | All |
| `disk_slow` | System | I/O latency injection | All |
| `process_kill` | Process | Terminates/restarts processes | All |
| `dns_fault` | Network | DNS delays, NXDOMAIN spoofing, blackholing | All |
| `fd_exhaustion` | System | File descriptor leak simulation (`EMFILE`/`ENFILE`) | All |
| `process_freeze` | Process | Execution pause (`SIGSTOP`/`SIGCONT` or OS suspend) | All |
| `clock_skew` | System | Time drift injection for TLS, JWT, & consensus | All |
| `disk_fill` | Storage | Ballast file allocation to trigger `ENOSPC` | All |
| `socket_corrupt` | Network | Bit-flipping and payload corruption in flight | All |
| `http_fault` | L7 Web | Synthetic 5xx, 429 rate limits, Slowloris | All |
| `nginx_fault` | Reverse Proxy | Upstream resets, 502/504 timeouts, SSL drops | All |
| `aws_fault` | Cloud | IMDS blackholing, S3 503, DynamoDB throttle, IAM drop | All |
| `azure_fault` | Cloud | ARM 429 throttling, CosmosDB RU exhaustion, Key Vault 403 | All |
| `cloudflare_fault` | Edge CDN | Cloudflare 520, 522/524 timeouts, Worker CPU, WAF 403 | All |
| `media_streaming_fault` | Media | HLS segment latency, DASH tag drop, RTSP frame drop, WebRTC SDP delay | All |
| `crypto_fault` | Security | TLS cert expiry, OCSP offline, entropy starvation | All |

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

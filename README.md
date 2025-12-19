# 🦀 Chaos Engineering Framework

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/NLlemain/chaos-engineering-rs)

**A lightweight, cross-platform chaos engineering framework for testing service resilience through controlled failure injection.**

Built in Rust for performance and safety. Test how your services handle real-world failures like network issues, resource exhaustion, and process crashes.

## ✨ Features

- **🌐 Cross-Platform**: Windows, macOS, Linux with platform-native chaos injection
- **⚡ High Performance**: Async Rust, ~15MB memory, <1% CPU overhead
- **🎯 7 Chaos Types**: Network latency, packet loss, TCP resets, CPU starvation, memory pressure, disk I/O, process kills
- **📋 YAML Configuration**: Declarative test scenarios with multi-phase support
- **🖥️ Web Dashboard**: Dark-themed UI for test management and monitoring
- **🔥 Load Testing**: Stress test HTTP, WebSocket, TCP, gRPC, HLS, RTMP endpoints
- **📊 Multiple Outputs**: CLI, JSON, Markdown, Prometheus metrics
- **🛡️ Safe by Design**: Input validation, no shell injection, clear privilege separation

## 🚀 Quick Start

### Prerequisites

- **Rust 1.70+** - [Install Rust](https://www.rust-lang.org/tools/install)
- **Windows**: No additional requirements
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

## 🖥️ Web Dashboard

Modern dark-themed web interface for chaos engineering:

- **Dashboard**: Real-time test status, system overview
- **Scenarios**: Browse and run YAML test scenarios
- **Load Testing**: Stress test any HTTP/WebSocket/TCP endpoint
- **Targets**: Save and manage your test endpoints
- **Results**: View test history with detailed metrics

### Load Testing Your Apps

Go to **Load Test** page and configure:

| Field | Description |
|-------|-------------|
| Target Type | HTTP, WebSocket, TCP, gRPC, HLS, RTMP |
| URL | Your endpoint (e.g., `http://localhost:3000/api`) |
| Concurrent Users | Parallel connections |
| Requests/Second | Target throughput |
| Duration | Test length in seconds |
| Ramp-up | Gradual load increase time |

**Supported Protocols:**
- **HTTP/HTTPS** - REST APIs, web apps
- **WebSocket** - Real-time feeds, chat
- **TCP** - Raw socket connections
- **gRPC** - gRPC services
- **HLS** - HTTP Live Streaming
- **RTMP** - Video streaming servers

## 📦 Chaos Injectors

| Injector | Description | Platform |
|----------|-------------|----------|
| `network_latency` | Adds delay to packets (mean + jitter) | All |
| `packet_loss` | Randomly drops packets | All |
| `tcp_reset` | Terminates TCP connections | All |
| `cpu_starvation` | Saturates CPU at specified intensity | All |
| `memory_pressure` | Allocates memory to target % | All |
| `disk_slow` | I/O latency injection | All |
| `process_kill` | Terminates/restarts processes | All |

## 📝 Test Scenarios

```yaml
name: "HTTP Service Resilience Test"
targets:
  - name: "web_api"
    type: "process"
    process_name: "axum_http_service"

phases:
  - name: "baseline"
    duration: "30s"
    
  - name: "network_stress"
    duration: "60s"
    injections:
      - type: "network_latency"
        target: "web_api"
        delay: "100ms"
        jitter: "20ms"
  
  - name: "resource_stress"
    duration: "60s"
    parallel: true
    injections:
      - type: "cpu_starvation"
        intensity: 0.7
      - type: "memory_pressure"
        target_usage: 0.8
        
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
├── chaos_core/        Injection engine
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
- **No Shell Injection**: Uses safe Rust `Command` API
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

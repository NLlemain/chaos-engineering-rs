# Roadmap

The roadmap favors credibility, recovery, and protocol depth over an inflated injector count. Priorities can be discussed in [GitHub Discussions](https://github.com/Ninian-Lemain/chaos-engineering-rs/discussions).

## v0.2 - Credibility And CI

- [x] Stable, experimental, and planned injector states.
- [x] Dependency, permission, and target validation.
- [x] Measurable effects and disruption/restoration integration tests.
- [x] Recovery journal, `recover`, and emergency `stop-all`.
- [x] Rootless TCP, HTTP, DNS, TLS, and AI API faults.
- [x] Docker and Docker Compose targeting.
- [x] DuckDB, SQLite, PostgreSQL, and MySQL failure paths.
- [x] Prometheus, OpenTelemetry, SLO gates, and report comparison.
- [x] Downloadable protocol scenario catalog.
- [x] Cross-platform release binaries, GitHub Action, container, and package-manager templates.

## v0.3 - Protocol Evidence

- [ ] Add opt-in integration suites with real PostgreSQL, MySQL, Kafka, RabbitMQ, NATS, MQTT, and S3-compatible targets.
- [ ] Promote stable scenario packs only after disruption and restoration assertions pass in CI.
- [ ] Add packet-level media fixtures for HLS segment loss and WebRTC keyframe-loss validation.
- [ ] Add OAuth/JWKS fixtures with certificate rotation and controllable clock sources.
- [ ] Add signed, versioned pack indexes with compatibility constraints.
- [ ] Add machine-readable `doctor --json` and capability-matrix generation.

## v0.4 - Distributed Experiments

- [ ] Remote agents with mutually authenticated control channels.
- [ ] Coordinated multi-target phases and blast-radius limits.
- [ ] Kubernetes workload selection and reversible pod/network policies.
- [ ] Central experiment history with reproducible seeds and artifact retention.
- [ ] Policy controls for allowed injectors, targets, schedules, and SLO budgets.

## Contribution Areas

Good first issues focus on one pack, fixture, documentation gap, or integration assertion. Core injector proposals must explain why existing rootless primitives and scenario packs cannot express the effect.

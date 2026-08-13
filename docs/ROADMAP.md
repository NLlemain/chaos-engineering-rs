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

- [x] Add opt-in integration suites with real PostgreSQL, MySQL, Kafka, RabbitMQ, NATS, MQTT, and S3-compatible targets.
- [x] Promote stable scenario packs only after disruption and restoration assertions pass in CI.
- [x] Add packet-level media fixtures for HLS segment loss and WebRTC keyframe-loss validation.
- [x] Add OAuth/JWKS fixtures with certificate rotation and controllable clock sources.
- [x] Add signed, versioned pack indexes with compatibility constraints.
- [x] Add machine-readable `list --json` and `doctor --json` output.
- [x] Generate the checked-in capability matrix from the runtime registry during releases.

## v0.4 - Distributed Experiments

- [x] Remote agents with mutually authenticated control channels.
- [x] Coordinated multi-target phases and blast-radius limits.
- [x] Kubernetes workload selection and reversible pod/network policies.
- [x] Central experiment history with reproducible seeds and artifact retention.
- [x] Policy controls for allowed injectors, targets, schedules, and SLO budgets.

## v0.5 - Market Systems Evidence

- [ ] Import PCAP, ITCH, OUCH, and FIX captures into the deterministic replay schema.
- [ ] Model exchange clocks, hardware timestamps, queueing delay, and clock-domain drift.
- [ ] Assert cross-venue failover, cancel-on-disconnect, kill-switch, position, and notional invariants.
- [ ] Attribute tail latency across network, gateway, risk, matching, and strategy stages.
- [ ] Add counterfactual replay reports for fill, slippage, exposure, and PnL divergence.

## Contribution Areas

Good first issues focus on one pack, fixture, documentation gap, or integration assertion. Core injector proposals must explain why existing rootless primitives and scenario packs cannot express the effect.

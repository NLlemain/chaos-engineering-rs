# Changelog

All notable changes are documented here. This project follows semantic versioning.

## 0.2.1 - 2026-08-02

### Fixed

- Database I/O pressure now uses a bounded rolling write set and responds promptly to cancellation, preventing recovery from hanging behind long cycle delays or storage synchronization stalls.
- Homebrew tap CI now validates installation on both Apple Silicon and Intel runners through the same tap flow users run locally.

## 0.2.0 - 2026-08-02

### Added

- Runtime capability registry with stable, experimental, and planned injector states.
- Dependency and permission validation through `doctor` and `dry-run`.
- Recovery journal, interrupted-run recovery, and emergency `stop-all` cleanup.
- Rootless directional TCP proxy with latency, bandwidth, timeout, slow-close, data-limit, partition, corruption, duplication, reordering, and connection-limit faults.
- Rootless DNS and deterministic TLS fault endpoints.
- Provider-aware HTTP and AI proxy for OpenAI-compatible, Azure OpenAI, Anthropic, Gemini, OpenRouter, Ollama, Mistral, Groq, Cohere, Together, and vLLM traffic.
- Real Docker and Docker Compose pause, stop, kill, and restart targeting.
- Local DuckDB/SQLite file faults plus PostgreSQL/MySQL proxy scenarios.
- Windows service, file-lock, handle-pressure, and named-pipe faults.
- Prometheus endpoint, OTLP/HTTP export, SLO assertions, CI failure gates, and baseline-versus-chaos report comparison.
- Searchable downloadable scenario catalog covering AI, authentication, media, databases, queues, IoT, object storage, containers, network, and Windows.
- Reusable GitHub Action, multi-platform release binaries, GHCR image workflow, GitHub artifact attestations, and Homebrew/WinGet release templates.

### Changed

- Successful injection now requires a measurable real-world effect.
- Planned injectors fail closed instead of reporting simulated success.
- CPU, memory, disk, file-descriptor, process, and recovery behavior has stricter validation and integration coverage.
- Repository identity and links now consistently use `Ninian-Lemain`.

### Fixed

- Cross-platform filesystem arithmetic and Unix portability in CI.
- Web load-test rate and duration handling.
- Scenario parameter application and injection-failure reporting.
- Dashboard injector count and copyright year.

## 0.1.0

- Initial multi-crate chaos framework, CLI, scenarios, metrics, targets, and web dashboard.

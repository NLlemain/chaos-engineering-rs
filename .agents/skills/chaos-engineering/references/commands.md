# Chaos CLI Commands Reference

Complete reference for all `chaos` commands and options.

## Core Commands

### `chaos doctor`
Checks dependencies, operating system permissions, and recovery journal state.
- `--json`: Output structured JSON check results for automated CI gates.

### `chaos list`
Lists all operating-system and proxy injectors registered in the binary.
- `--json`: Output structured JSON array with required capabilities and status.

### `chaos validate <PATH>`
Validates the syntax, structure, and injector schemas of a scenario YAML file.

### `chaos dry-run <PATH>`
Simulates a scenario execution without applying disruptive faults. Checks target connectivity, permissions, and duration timers.

### `chaos run <PATH>`
Executes a scenario YAML, running each phase and evaluating SLO assertions.
- `--output-json <PATH>`: Write execution summary and metric results to JSON.
- `--output-markdown <PATH>`: Write human-readable test report to markdown.
- `--prometheus-port <PORT>`: Bind Prometheus scraper endpoint (e.g. `9898`).
- `--otlp-endpoint <URL>`: Export metrics over OTLP/HTTP endpoint.

### `chaos report <PATH>`
Generates formatted reports from an output metrics JSON file.
- `--compare <BASELINE_PATH>`: Compare experiment against a baseline run.
- `--format <text|markdown|json>`: Output format (default: `text`).
- `--output <PATH>`: Write output to file instead of stdout.

### `chaos recover`
Recovers active or interrupted injections recorded in the recovery journal.
- `--journal <PATH>`: Custom journal file path (default: `~/.chaos-engineering/recovery.json`).

### `chaos stop-all`
Emergency stop command. Immediately restores all journaled injections.

---

## Rootless Proxies

### `chaos proxy`
Starts a directional TCP proxy for injecting network latency, jitter, bandwidth caps, drops, and corruption.
- `--listen <HOST:PORT>`: Proxy bind address.
- `--upstream <HOST:PORT>`: Target upstream address.
- `--direction <upstream|downstream|both>`: Traffic direction for fault injection.
- `--latency <DURATION>`: Added latency (e.g. `250ms`, `1s`).
- `--jitter <DURATION>`: Latency jitter (e.g. `40ms`).
- `--bandwidth <BYTES_PER_SEC>`: Bandwidth throttle in bytes per second.
- `--drop-rate <0.0-1.0>`: Probability of packet drop.
- `--corrupt-rate <0.0-1.0>`: Probability of byte corruption.

### `chaos ai-proxy`
Provider-aware proxy for AI APIs, LLM streams, and HTTP services.
- `--provider <open-ai|azure-openai|anthropic|gemini|openrouter|ollama|mistral|groq|cohere|together|vllm>`: Provider API dialect.
- `--listen <HOST:PORT>`: Local listener address.
- `--upstream <URL>`: Upstream API URL.
- `--latency <DURATION>`: Upstream latency.
- `--stream-delay <DURATION>`: Delay between streaming tokens / SSE chunks.
- `--stream-abort <COUNT>`: Abort streaming response after N tokens.
- `--error-rate <0.0-1.0>`: Inject 429 / 500 error responses with probability.

### `chaos dns-proxy`
Rootless DNS resolver for injecting DNS delays, packet drops, or spoofed answers.
- `--listen <HOST:PORT>`: DNS listener address (e.g. `127.0.0.1:15353`).
- `--upstream <HOST:PORT>`: Upstream DNS server.
- `--spoof <DOMAIN=IP>`: Override domain resolution.

### `chaos tls-endpoint`
Starts a TLS server endpoint with configurable handshake failures, expired certificates, or incomplete trust chains.

---

## Infrastructure and Workloads

### `chaos container`
Direct manipulation of Docker and Docker Compose services.
- `--id <CONTAINER>`: Docker container name or ID.
- `--compose-file <PATH>`: Path to `docker-compose.yaml`.
- `--compose-service <NAME>`: Target Compose service name.
- `--action <pause|unpause|restart|stop|kill>`: Container lifecycle action.
- `--duration <DURATION>`: Duration for temporary states like `pause`.

### `chaos database`
Direct fault injection for local SQLite and DuckDB database files.
- `--engine <sqlite|duckdb>`: Target database engine.
- `--file <PATH>`: Target database file path.
- `--mode <read-only|unavailable|io-pressure>`: Fault mode.
- `--duration <DURATION>`: Duration of the degradation.

---

## Data Pipelines and Trading

### `chaos pipeline replay <RECORDS_PATH>`
Replays a JSON Lines stream through a zero-buffer rendezvous channel.
- `--fault-plan <PATH>`: Replay fault plan YAML (drops, stalls, reordering, schema corruption).
- `--budget <PATH>`: Invariant budget YAML (max loss, ordering, delivery guarantees).
- `--output <PATH>`: Output evidence JSON path.

### `chaos hft replay <EVENTS_PATH>`
Replays high-frequency market event streams under deterministic faults.
- `--fault-plan <PATH>`: Fault plan YAML (sequence gaps, duplicates, latency).
- `--budget <PATH>`: Invariant budget YAML.
- `--output <PATH>`: Output evidence JSON path.

### `chaos hft fix <FIX_PATH>`
Mutates FIX protocol messages for trading venue resilience tests.
- `--fault-plan <PATH>`: FIX fault plan YAML.
- `--output <PATH>`: Faulted FIX output path.

---

## Catalog Packs

### `chaos pack list`
Lists available scenario packs in the catalog.
- `--category <CATEGORY>`: Filter by category (`ai`, `authentication`, `containers`, `data-pipelines`, `databases`, `iot`, `media`, `network`, `object-storage`, `observability`, `queues`, `trading`, `windows`).
- `--search <QUERY>`: Search packs by keyword.

### `chaos pack show <NAME>`
Displays detailed YAML definition and instructions for a pack.

### `chaos pack install <NAME>`
Installs a scenario pack to a destination directory.
- `--output <DIR>`: Output directory path.

---

## Dashboard and Distributed

### `chaos serve`
Starts the embedded local web dashboard.
- `--host <HOST>`: Bind host (default: `127.0.0.1`).
- `--port <PORT>`: Bind port (default: `8080`).
- `--scenarios-dir <PATH>`: Directory containing scenario files.
- `--results-dir <PATH>`: Directory containing results.

### `chaos agent`
Runs a mutually authenticated mTLS agent for remote experiment coordination.

### `chaos distributed <MANIFEST_PATH>`
Coordinates distributed experiments across remote agents with blast-radius enforcement.

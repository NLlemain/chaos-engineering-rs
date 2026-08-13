# Quick Start

## Install

Download an archive from [GitHub Releases](https://github.com/Ninian-Lemain/chaos-engineering-rs/releases), or build the CLI with Rust 1.82 or newer:

```bash
git clone https://github.com/Ninian-Lemain/chaos-engineering-rs
cd chaos-engineering-rs
cargo build --locked --release -p chaos_cli
```

The binary is `target/release/chaos` on Unix and `target\release\chaos.exe` on Windows. The examples below assume it is on `PATH`.

## Inspect The Environment

```bash
chaos list
chaos doctor
chaos validate scenarios/slo_gate.yaml
chaos dry-run scenarios/slo_gate.yaml
```

`doctor` intentionally exits nonzero when an operational injector is blocked. This makes missing Docker access, system commands, or elevated permissions visible in CI.

## Run An SLO Gate

Start the service under test, update the probe URL in `scenarios/slo_gate.yaml`, then run:

```bash
chaos run scenarios/slo_gate.yaml \
  --output-json test_results/chaos.json \
  --output-markdown test_results/chaos.md \
  --prometheus-port 9898
```

The command exits nonzero when an SLO assertion fails. Metrics remain available at `http://127.0.0.1:9898/metrics` while the experiment runs.

## Add A Rootless Dependency Fault

Place the proxy between an application and PostgreSQL:

```bash
chaos proxy \
  --listen 127.0.0.1:15432 \
  --upstream 127.0.0.1:5432 \
  --direction downstream \
  --latency 250ms \
  --jitter 40ms \
  --bandwidth 65536
```

Point the application at port `15432`. The proxy also supports partitions, connection timeouts, slow closes, data limits, duplication, reordering, corruption, and max-connection pressure; use `chaos proxy --help` for the full set.

## Test HTTP And AI APIs

```bash
chaos ai-proxy \
  --provider open-ai \
  --listen 127.0.0.1:18080 \
  --upstream https://api.openai.com \
  --stream-abort 3
```

Provider profiles cover OpenAI, Azure OpenAI, Anthropic, Gemini, OpenRouter, Ollama, Mistral, Groq, Cohere, Together, and vLLM. Faults include delayed tokens, broken streams, malformed tool calls, 429 storms, context truncation, status replacement, truncated bodies, and malformed headers.

## Target Docker And Compose

```bash
chaos container --id my-api --action pause --duration 10s
chaos container --compose-file compose.yaml --compose-service api --action restart
```

The Docker CLI and an accessible daemon are required. Container state is journaled for cleanup.

## Target Local Databases

```bash
chaos database --engine duckdb --file ./data/app.duckdb --mode unavailable --duration 10s
chaos database --engine sqlite --file ./data/app.sqlite --mode read-only --duration 10s
```

DuckDB and SQLite file faults are local. PostgreSQL and MySQL disconnects, slow queries, and pool pressure use the dependency proxy and database scenario pack.

## Browse Scenario Packs

```bash
chaos pack list --category ai
chaos pack show database-postgres-disconnect
chaos pack install database-postgres-disconnect --output ./scenarios
```

The catalog also contains authentication, queues, MQTT, media, object storage, containers, trading, network, and Windows packs.

## Replay Market And FIX Faults

```bash
chaos hft replay tests/hft-evidence/market-events.jsonl \
  --fault-plan tests/hft-evidence/sequence-gap.yaml \
  --budget tests/hft-evidence/invariants.yaml \
  --output test_results/hft-evidence.json

chaos hft fix tests/hft-evidence/orders.fix \
  --fault-plan tests/hft-evidence/fix-faults.yaml \
  --output test_results/faulted-orders.fix
```

The replay succeeds only when chaos is measurable and restoration exactly reproduces the baseline state digest. See [docs/HFT.md](docs/HFT.md) for the event schema, invariant budgets, and research direction.

## Coordinate Remote Agents

Create development certificates as described in [docs/DISTRIBUTED.md](docs/DISTRIBUTED.md), start each agent with its local policy, then run:

```bash
chaos distributed examples/distributed-experiment.yaml \
  --ca-cert certs/ca.pem \
  --cert certs/orchestrator.pem \
  --key certs/orchestrator-key.pem \
  --policy examples/distributed-policy.yaml

chaos history list
```

The manifest requests 50% blast radius across two simulated venue feeds, so no more than one target runs at once.

## Isolate Kubernetes Workloads

```bash
chaos dry-run examples/kubernetes-network-isolation.yaml
chaos run examples/kubernetes-network-isolation.yaml
```

The injector checks `kubectl auth can-i`, selects a deterministic subset of matching pods, verifies the temporary `NetworkPolicy`, and journals the labels and policy name for cleanup.

## Compare Baseline And Chaos

```bash
chaos report test_results/chaos.json \
  --compare test_results/baseline.json \
  --format markdown \
  --output test_results/comparison.md
```

Use `--otlp-endpoint http://127.0.0.1:4318/v1/metrics` on `chaos run` to export final metrics over OTLP/HTTP.

## Dashboard

```bash
chaos serve --host 127.0.0.1 --port 8080 \
  --scenarios-dir scenarios \
  --results-dir test_results
```

Open `http://127.0.0.1:8080` to browse scenarios, launch tests, run load probes, and inspect results.

## Recover Safely

```bash
chaos recover
chaos stop-all
```

Recovery uses `~/.chaos-engineering/recovery.json` by default. Run `stop-all` as the emergency command when an experiment was interrupted or its original process disappeared.

## Minimal Scenario

```yaml
name: dependency_latency
description: Add latency to a downstream without administrator privileges.
duration: 30s
phases:
  - name: degraded_database_link
    duration: 30s
    injections:
      - type: dependency_proxy
        target:
          address: 127.0.0.1:5432
        listen: 127.0.0.1:15432
        direction: downstream
        latency: 250ms
        jitter: 40ms
```

Always run `chaos dry-run FILE` before using a new scenario against a shared environment.

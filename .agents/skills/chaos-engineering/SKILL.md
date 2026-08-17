---
name: chaos-engineering
description: >-
  Execute chaos engineering experiments, fault injection, resilience testing,
  and SLO validation using chaos-engineering-rs (chaos CLI). Use when running
  fault injection, chaos tests, network latency/loss proxies, container/database
  disruptions, zero-buffer pipeline replays, HFT/FIX market data tests, or SLO gates.
---

# Chaos Engineering Skill

Use this skill to execute reproducible, evidence-driven chaos experiments, fault injection, and SLO validation across infrastructure, APIs, databases, containers, data pipelines, and trading systems.

## Prerequisites

Ensure the `chaos` CLI is installed and accessible on `PATH`:
- Windows: `winget install --id NinianLemain.ChaosEngineeringRs --exact` or `target\release\chaos.exe`
- macOS / Linux: `brew install Ninian-Lemain/chaos-engineering/chaos-engineering-rs` or `target/release/chaos`
- From source: `cargo build --locked --release -p chaos_cli`

---

## Agent Workflow

Follow this procedure when executing chaos experiments:

### 1. Preflight Inspection

Always inspect available injectors and system capabilities before designing or executing tests:

```bash
# Check dependencies, permissions, and recovery journal
chaos doctor

# Machine-readable output for programmatic inspection
chaos doctor --json

# List available injectors and required capabilities (e.g. CAP_NET_ADMIN, Docker)
chaos list --json
```

If `doctor` reports missing permissions (e.g. Docker daemon unreachable or non-root network capabilities), select rootless injectors like `dependency_proxy`, `ai-proxy`, `dns-proxy`, or application-level scenario packs.

### 2. Select or Author a Scenario

Browse the scenario pack catalog or create a custom scenario YAML:

```bash
# Browse packs by category (ai, databases, data-pipelines, containers, network, trading, queues)
chaos pack list --category data-pipelines
chaos pack list --search postgres

# Show pack manifest and parameters
chaos pack show database-postgres-disconnect

# Install pack into local directory
chaos pack install database-postgres-disconnect --output ./scenarios
```

For custom scenarios, see [references/scenarios.md](./references/scenarios.md).

### 3. Validate and Dry-Run

Always validate syntax and verify preconditions before running against any environment:

```bash
# Validate syntax and schema
chaos validate scenarios/my_scenario.yaml

# Test permissions and target connectivity without injecting faults
chaos dry-run scenarios/my_scenario.yaml
```

### 4. Execute Scenario with SLO Gate

Run the scenario and capture machine-readable results. The command exits nonzero if any SLO assertion fails:

```bash
chaos run scenarios/my_scenario.yaml \
  --output-json test_results/chaos.json \
  --output-markdown test_results/chaos.md \
  --prometheus-port 9898
```

Key flags:
- `--output-json <PATH>`: Write structured execution results and metrics.
- `--output-markdown <PATH>`: Generate a human-readable execution summary.
- `--prometheus-port <PORT>`: Expose live Prometheus metrics during the run.
- `--otlp-endpoint <URL>`: Export OpenTelemetry metrics over OTLP/HTTP.

### 5. Generate Comparison and Analysis

Compare results against baseline runs:

```bash
chaos report test_results/chaos.json \
  --compare test_results/baseline.json \
  --format markdown \
  --output test_results/comparison.md
```

### 6. Verify Cleanup and Safety

Every recoverable fault is journaled to `~/.chaos-engineering/recovery.json`. If an experiment fails or is interrupted:

```bash
# Recover un-restored journal effects
chaos recover

# Emergency cleanup of all journaled state
chaos stop-all
```

---

## Common Workflows

### Rootless HTTP and AI API Faults

Test upstream timeouts, token delays, 429 storms, or broken SSE streams without administrator permissions:

```bash
chaos ai-proxy \
  --provider open-ai \
  --listen 127.0.0.1:18080 \
  --upstream https://api.openai.com \
  --stream-abort 3
```

Providers supported: `open-ai`, `azure-openai`, `anthropic`, `gemini`, `openrouter`, `ollama`, `mistral`, `groq`, `cohere`, `together`, `vllm`.

### Rootless Dependency Proxy

Place a directional proxy between an application and its downstream database, queue, or API:

```bash
chaos proxy \
  --listen 127.0.0.1:15432 \
  --upstream 127.0.0.1:5432 \
  --direction downstream \
  --latency 250ms \
  --jitter 40ms \
  --bandwidth 65536
```

### Container and Database Faults

```bash
# Docker container action (pause, restart, stop, kill)
chaos container --id target-service --action pause --duration 10s

# Compose service restart
chaos container --compose-file compose.yaml --compose-service api --action restart

# Local database degradation
chaos database --engine sqlite --file ./data/app.sqlite --mode read-only --duration 15s
chaos database --engine duckdb --file ./data/app.duckdb --mode unavailable --duration 10s
```

### Zero-Buffer Pipeline Replay

Verify stream consumer backpressure and invariant budgets:

```bash
chaos pipeline replay tests/pipeline-evidence/records.jsonl \
  --fault-plan tests/pipeline-evidence/zero-buffer-stall.yaml \
  --budget tests/pipeline-evidence/budget.yaml \
  --output test_results/pipeline-evidence.json
```

### HFT and FIX Protocol Replay

```bash
# Deterministic market data replay with invariant check
chaos hft replay tests/hft-evidence/market-events.jsonl \
  --fault-plan tests/hft-evidence/sequence-gap.yaml \
  --budget tests/hft-evidence/invariants.yaml \
  --output test_results/hft-evidence.json

# FIX protocol message corruption
chaos hft fix tests/hft-evidence/orders.fix \
  --fault-plan tests/hft-evidence/fix-faults.yaml \
  --output test_results/faulted-orders.fix
```

---

## Detailed References

- [CLI Commands Reference](./references/commands.md)
- [Scenario Schema and Invariants](./references/scenarios.md)

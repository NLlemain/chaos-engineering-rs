# Zero-Buffer Pipeline Evidence

`records.jsonl` is deliberately protocol-neutral. The records could come from a
crypto WebSocket feed, CDC connector, telemetry exporter, queue consumer, or an
in-process stage boundary.

`zero-buffer-stall.yaml` proves producer backpressure with a capacity-zero
rendezvous channel. `integrity-faults.yaml` demonstrates sequence loss,
reordering, and JSON Pointer field corruption. Every run compares baseline,
faulted, and restored delivery state.

```bash
chaos pipeline replay tests/pipeline-evidence/records.jsonl \
  --fault-plan tests/pipeline-evidence/zero-buffer-stall.yaml \
  --budget tests/pipeline-evidence/budget.yaml \
  --output pipeline-evidence.json
```

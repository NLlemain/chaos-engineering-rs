# HFT And Market-Microstructure Fault Lab

The HFT lab tests resilience properties that generic latency proxies cannot understand. It is intended for simulated venues, captured fixtures, CI, interview projects, and pre-production trading infrastructure, never live capital without an independent safety review.

## Market Event Schema

One JSON object per line represents a venue and symbol stream. Supported event types are book updates, trades, order acknowledgements, cancel acknowledgements, and heartbeats. Prices use integer ticks and timestamps use integer nanoseconds to avoid floating-point ambiguity.

Seeded faults include deterministic and probabilistic drops, duplication, adjacent reordering, timestamp skew, acknowledgement delay, quantity corruption, and sequence-bounded venue partitions.

## Invariants

Replay reports:

- per-venue and per-symbol sequence gaps, duplicates, and out-of-order messages
- stale timestamps
- crossed order books
- rejected orders
- p50 and p99 acknowledgement latency in nanoseconds
- a deterministic SHA-256 digest of final book state

The baseline, faulted stream, and restored stream use the same input. A successful evidence run proves that the baseline is valid, the selected fault changed an observable result, and restoration exactly reproduces baseline state.

## FIX Session Faults

The FIX parser accepts SOH or pipe delimiters and produces a correct BodyLength and CheckSum when encoding. Fault plans can create sequence gaps, duplicate messages, set `PossDupFlag(43)`, corrupt checksums, or transform an order into an execution reject.

```bash
chaos hft fix tests/hft-evidence/orders.fix \
  --fault-plan tests/hft-evidence/fix-faults.yaml \
  --output faulted-orders.fix
```

## Research Direction

The next market-systems work is deliberately narrow and evidence-focused: PCAP/ITCH/OUCH import, deterministic exchange-clock models, cross-venue failover and cancel-on-disconnect assertions, strategy/risk invariant plugins, and latency attribution that separates network, queueing, matching, and application time.

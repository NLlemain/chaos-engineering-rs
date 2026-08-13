# HFT Evidence Fixtures

These deterministic fixtures exercise market-data sequence gaps, latency and book invariants, exact post-fault restoration, and FIX session faults. Run them without live market connectivity:

```bash
chaos hft replay tests/hft-evidence/market-events.jsonl \
  --fault-plan tests/hft-evidence/sequence-gap.yaml \
  --budget tests/hft-evidence/invariants.yaml \
  --output hft-evidence.json

chaos hft fix tests/hft-evidence/orders.fix \
  --fault-plan tests/hft-evidence/fix-faults.yaml \
  --output faulted-orders.fix
```

The replay command succeeds only when the baseline is valid, the fault has a measurable effect, and replay after recovery exactly matches the baseline digest.

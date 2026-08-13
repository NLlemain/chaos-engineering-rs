# Protocol Evidence Suites

These opt-in suites run disposable real services and use protocol-aware probes. Each suite proves a healthy baseline, applies `container_fault` through the public CLI, proves the protocol is disrupted, and proves both the container state and protocol are restored.

Run one suite on Linux with Docker Compose:

```bash
tests/protocol-evidence/run.sh postgres
```

The GitHub Actions workflow accepts one target or the complete matrix. Services and volumes are removed even when an assertion fails.

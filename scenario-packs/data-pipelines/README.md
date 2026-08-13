# Data Pipeline Failure Pack

These plans run with `chaos pipeline replay` against protocol-neutral JSON Lines
records. The replay engine uses a capacity-zero rendezvous channel: producers
cannot enqueue ahead of consumers, so a consumer stall is measured directly as
producer blocking.

Use the generic plans for in-process stages, WebSocket feeds, CDC events,
telemetry, queue consumers, ETL/ELT jobs, and streaming analytics. Stable plans
have deterministic disruption and exact-restoration evidence in workspace CI.


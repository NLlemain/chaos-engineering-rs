# Observability Pipeline Failure Pack

These plans exercise failure modes inside telemetry pipelines rather than only
stopping the collector. They target high-cardinality attributes and event-time
regressions, both of which can leave a pipeline technically reachable while its
cost, memory use, temporality, or query results degrade.


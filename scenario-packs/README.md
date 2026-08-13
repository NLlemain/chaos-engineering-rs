# Chaos Scenario Packs

Curated, downloadable scenarios and offline fault plans for `chaos-engineering-rs`. The catalog covers APIs, authentication, containers, data pipelines, databases, media, queues, IoT/MQTT, storage, observability, trading, network protocols, and Windows-specific failures.

Search and install packs with:

```bash
chaos pack list
chaos pack show hls-stale-manifest
chaos pack install hls-stale-manifest
```

Every downloadable YAML file declares a `scenario` or `pipeline_fault_plan` kind and is checked with its production parser during workspace tests. Stable packs map to CI evidence for both disruption and restoration. See `catalog.json` for status and runtime requirements.

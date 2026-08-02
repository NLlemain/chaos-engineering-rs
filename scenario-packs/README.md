# Chaos Scenario Packs

Curated, downloadable scenarios for `chaos-engineering-rs`. The catalog covers AI APIs, authentication, containers, local and network databases, media streaming, queues, IoT/MQTT, object storage, DNS, and Windows-specific failures.

Search and install packs with:

```bash
chaos pack list
chaos pack show hls-stale-manifest
chaos pack install hls-stale-manifest
```

Every downloadable YAML file is parsed and its configured injectors are built during workspace tests. See `catalog.json` for capability status and runtime requirements.

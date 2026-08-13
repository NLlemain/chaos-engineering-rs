# Trading And HFT Packs

These packs target the links around exchange gateways, market-data handlers, order routers, and risk services. Route only disposable or simulated traffic through the configured local listener.

Use `chaos hft replay` for deterministic sequence, timestamp, book, acknowledgement-latency, and restoration evidence. Use `chaos hft fix` for message-aware FIX session faults. The YAML packs below are network experiments suitable for existing applications without embedding a trading protocol engine in the core injector registry.

# Architecture

Clients -> Axum HTTP server -> Gateway kernel -> Capability runtime pipeline -> provider adapters -> client.

Runtime pipeline (default):

- identity
- policy
- budget_guard
- pii_filter
- semantic_router
- tool_mcp
- provider_router
- wasm

Core stack:

- Rust 2024
- Tokio
- Axum + Tower
- reqwest
- Wasmtime
- serde + YAML
- tracing (JSON)
- prometheus

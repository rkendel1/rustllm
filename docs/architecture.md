# Architecture

Clients -> Axum HTTP server -> auth/rate-limit -> Wasm hook chain -> provider router/fallback -> provider adapters -> response hooks -> client.

Core stack:

- Rust 2024
- Tokio
- Axum + Tower
- reqwest
- Wasmtime
- serde + YAML
- tracing (JSON)
- prometheus

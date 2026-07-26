# aether (rustllm)

A high-performance, single-binary AI gateway kernel with a capability runtime.

## MVP features

- OpenAI-compatible `POST /v1/chat/completions`
- Streaming and non-streaming pass-through
- Health endpoints (`/health`, `/healthz`)
- Multi-provider routing with model aliases and fallback
- Providers: OpenAI-compatible, Anthropic, local OpenAI-compatible
- Retries with exponential backoff + connect/total timeout controls
- API key auth (virtual keys)
- Basic global/per-key rate limiting
- Prometheus metrics (`/metrics`)
- Request ID propagation (`x-request-id`)
- Token + estimated cost counters
- Capability runtime pipeline (identity, policy, routing, budget, guardrails, tools, providers, wasm)
- Wasm capabilities via the same runtime contract as native capabilities

## Run

```bash
cargo run -- config.example.yaml
```

Set provider secrets as env vars used by config:

- `OPENAI_API_KEY`
- `ANTHROPIC_API_KEY`

## Wasm plugin ABI

Each plugin must export:

- `memory`
- `alloc(i32) -> i32`
- `dealloc(i32, i32)`
- hook functions: `on_auth`, `on_request`, `on_response`, `on_stream_chunk`
- hook signature: `(ptr: i32, len: i32) -> i64` where return packs output pointer/len as `(ptr << 32) | len`

Input and output payloads are JSON. See `plugins/keyword_guardrail` for a working reference.

## Capability runtime

Requests and responses flow through the configurable capability pipeline:

`identity -> policy -> budget_guard -> pii_filter -> semantic_router -> tool_mcp -> provider_router -> wasm`

Each capability implements a shared contract (`on_request`, `on_response`) and receives a standard runtime context containing identity, metadata, budget, and policy state.

## Build example plugin

```bash
cd plugins/keyword_guardrail
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/keyword_guardrail.wasm ../../plugins/keyword_guardrail.wasm
```

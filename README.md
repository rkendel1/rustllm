# aether (rustllm)

A high-performance, single-binary AI gateway kernel with a capability runtime.

## MVP features

- OpenAI-compatible `POST /v1/chat/completions`
- Dry-run planning: `POST /v1/chat/completions?dry_run=true`
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
- Capability graph runtime with dependency-aware execution planning and immutable execution intents
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

Requests and responses flow through a dependency-aware capability graph:

`identity -> policy -> semantic_router -> (budget_guard, pii_filter, tool_mcp) -> provider_router -> wasm`

Each capability implements a shared contract (`on_request`, `on_response`), declares a manifest, and receives a runtime context plus capability state with shared runtime facts.

`GET /debug/plan` returns planner diagnostics including execution order, missing dependencies, and parallel groups.

`GET /debug/intent` returns the latest execution intent and capability decision log.

## Build example plugin

```bash
cd plugins/keyword_guardrail
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/keyword_guardrail.wasm ../../plugins/keyword_guardrail.wasm
```

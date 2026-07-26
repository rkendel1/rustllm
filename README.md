# aether (rustllm)

A high-performance, single-binary LLM/AI gateway written in Rust with Wasm plugin hooks.

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
- Wasm plugin hooks: `on_auth`, `on_request`, `on_response`, `on_stream_chunk`

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

## Build example plugin

```bash
cd plugins/keyword_guardrail
cargo build --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/keyword_guardrail.wasm ../../plugins/keyword_guardrail.wasm
```

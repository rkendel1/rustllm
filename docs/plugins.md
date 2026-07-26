# Plugins

Plugins are sandboxed Wasm modules loaded from config.

Supported hooks:

- on_auth
- on_request
- on_response
- on_stream_chunk

Contract:

- Host sends JSON `{hook, config, payload}` bytes
- Plugin returns JSON `{allow, reject_reason?, body?}` bytes
- Rejections (`allow=false`) stop request/response flow

See `plugins/keyword_guardrail` for a reference implementation.

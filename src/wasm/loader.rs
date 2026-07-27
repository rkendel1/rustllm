use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde_json::json;
use wasmtime::{Engine, Instance, Module, Store};

use crate::{
    config::PluginConfig,
    kernel::{
        capability::{Capability, CapabilityFuture, CapabilityResult},
        context::{RequestContext, ResponseContext},
    },
};

use super::{
    abi::{Hook, HookResult},
    host::{read_memory, write_memory},
};

pub struct PluginManager {
    engine: Engine,
    plugins: Vec<LoadedPlugin>,
}

struct LoadedPlugin {
    name: String,
    module: Module,
    config: serde_json::Value,
}

impl PluginManager {
    pub fn from_config(config: &[PluginConfig]) -> Result<Self> {
        let engine = Engine::default();
        let mut plugins = Vec::with_capacity(config.len());

        for plugin in config {
            let path = Path::new(&plugin.path);
            let module = Module::from_file(&engine, path).with_context(|| {
                format!(
                    "failed to compile plugin '{}' from {}",
                    plugin.name,
                    path.display()
                )
            })?;
            plugins.push(LoadedPlugin {
                name: plugin.name.clone(),
                module,
                config: plugin.config.clone(),
            });
        }

        Ok(Self { engine, plugins })
    }

    pub fn execute(&self, hook: Hook, input: &serde_json::Value) -> Result<HookResult> {
        let mut current = HookResult {
            allow: true,
            reject_reason: None,
            body: Some(input.clone()),
        };

        for plugin in &self.plugins {
            let envelope = json!({
                "hook": hook.export_name(),
                "config": plugin.config,
                "payload": current.body.clone().unwrap_or_default()
            });

            let result = self.invoke_plugin(plugin, hook, &envelope)?;
            if !result.allow {
                return Ok(result);
            }

            if let Some(body) = result.body {
                current.body = Some(body);
            }
        }

        Ok(current)
    }

    fn invoke_plugin(
        &self,
        plugin: &LoadedPlugin,
        hook: Hook,
        input: &serde_json::Value,
    ) -> Result<HookResult> {
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &plugin.module, &[])
            .with_context(|| format!("failed to instantiate plugin {}", plugin.name))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("plugin {} missing memory export", plugin.name))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .with_context(|| format!("plugin {} missing alloc export", plugin.name))?;
        let dealloc = instance
            .get_typed_func::<(i32, i32), ()>(&mut store, "dealloc")
            .with_context(|| format!("plugin {} missing dealloc export", plugin.name))?;
        let hook_fn = instance
            .get_typed_func::<(i32, i32), i64>(&mut store, hook.export_name())
            .with_context(|| {
                format!(
                    "plugin {} missing '{}' export",
                    plugin.name,
                    hook.export_name()
                )
            })?;

        let input_bytes = serde_json::to_vec(input)?;
        let input_ptr = alloc.call(&mut store, input_bytes.len() as i32)?;
        write_memory(&mut store, &memory, input_ptr, &input_bytes)?;

        let packed = hook_fn.call(&mut store, (input_ptr, input_bytes.len() as i32))?;
        let output_ptr = (packed >> 32) as i32;
        let output_len = packed as i32;
        if output_ptr < 0 || output_len < 0 {
            return Err(anyhow!(
                "plugin {} returned invalid pointer/len",
                plugin.name
            ));
        }

        let output = read_memory(&mut store, &memory, output_ptr, output_len as usize)?;
        dealloc.call(&mut store, (input_ptr, input_bytes.len() as i32))?;
        dealloc.call(&mut store, (output_ptr, output_len))?;

        let parsed: HookResult = serde_json::from_slice(&output)
            .with_context(|| format!("plugin {} output was invalid json", plugin.name))?;
        Ok(parsed)
    }
}

pub struct WasmCapability {
    plugins: PluginManager,
}

impl WasmCapability {
    pub fn new(plugins: PluginManager) -> Self {
        Self { plugins }
    }
}

impl Capability for WasmCapability {
    fn id(&self) -> &'static str {
        "wasm"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            let payload = serde_json::to_value(&ctx.model)?;
            let result = self.plugins.execute(Hook::OnRequest, &payload)?;
            if !result.allow {
                return Ok(CapabilityResult::Deny {
                    message: result
                        .reject_reason
                        .unwrap_or_else(|| "request rejected by wasm capability".to_string()),
                    kind: "wasm_reject".to_string(),
                    status_code: 403,
                });
            }
            if let Some(body) = result.body {
                ctx.model = serde_json::from_value(body)
                    .context("wasm capability produced invalid request payload")?;
            }
            Ok(CapabilityResult::Continue)
        })
    }

    fn on_response<'a>(&'a self, ctx: &'a mut ResponseContext) -> CapabilityFuture<'a> {
        Box::pin(async move {
            let result = self.plugins.execute(Hook::OnResponse, &ctx.body)?;
            if !result.allow {
                return Ok(CapabilityResult::Deny {
                    message: result
                        .reject_reason
                        .unwrap_or_else(|| "response rejected by wasm capability".to_string()),
                    kind: "wasm_reject".to_string(),
                    status_code: 403,
                });
            }
            if let Some(body) = result.body {
                ctx.body = body;
            }
            Ok(CapabilityResult::Continue)
        })
    }
}

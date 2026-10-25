// SPDX-License-Identifier: MIT
//! No WASI, host imports, filesystem, environment or sockets are linked.
use cobble_plugin_api::{
    MAX_TOOL_FORM_BYTES, PluginError, ToolFormOutput, ToolFormRequest, ToolTextBlock,
};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
wasmtime::component::bindgen!({path:"../../contracts/tool-form",world:"tool-form"});
pub fn execute() -> Result<(), PluginError> {
    let mut input = std::io::stdin().lock();
    let bytes = super::read_frame(&mut input, cobble_plugin_api::MAX_COMPONENT_BYTES)?;
    let request: ToolFormRequest =
        serde_json::from_slice(&super::read_frame(&mut input, MAX_TOOL_FORM_BYTES)?)
            .map_err(|_| PluginError::new("PLUGIN_INVALID_TOOL_FORM", "Invalid tool input"))?;
    request.validate_envelope()?;
    if !bytes.starts_with(b"\0asm\x0d\0\x01\0") {
        return Err(PluginError::new(
            "PLUGIN_INVALID_PACKAGE",
            "Expected raw WebAssembly Component",
        ));
    }
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true)
        .cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::new(&config)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not initialize tool engine"))?;
    let component = Component::new(&engine, bytes)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_COMPONENT", "Tool compilation failed"))?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(64 * 1024 * 1024)
        .table_elements(10000)
        .instances(16)
        .memories(1)
        .tables(16)
        .trap_on_grow_failure(true)
        .build();
    let mut store: Store<StoreLimits> = Store::new(&engine, limits);
    store.limiter(|state| state);
    store
        .set_fuel(400_000_000)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not apply tool fuel budget"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let linker = Linker::new(&engine);
    let bindings = ToolForm::instantiate(&mut store, &component, &linker).map_err(|_| {
        PluginError::new(
            "PLUGIN_PERMISSION_DENIED",
            "Unsupported tool imports or initialization failure",
        )
    })?;
    let request = exports::launcher::tool_form::tool::Request {
        locale: request.locale,
        input_json: serde_json::to_string(&request.input)
            .map_err(|_| PluginError::new("PLUGIN_INVALID_TOOL_FORM", "Invalid tool input"))?,
    };
    let output = bindings
        .launcher_tool_form_tool()
        .call_run(&mut store, &request)
        .map_err(|_| PluginError::new("PLUGIN_TRAP", "Tool trapped or exceeded budget"))?
        .map_err(|failure| {
            cobble_plugin_api::tool_form_guest_failure(&failure.code, &failure.message)
        })?;
    let output = ToolFormOutput {
        title: output.title,
        blocks: output
            .blocks
            .into_iter()
            .map(|block| ToolTextBlock {
                label: block.label,
                text: block.text,
            })
            .collect(),
    };
    output.validate()?;
    super::send(&Ok(output))
}

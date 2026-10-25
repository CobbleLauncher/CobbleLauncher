// SPDX-License-Identifier: MIT
use cobble_plugin_api::{
    MAX_COMPONENT_BYTES, MAX_FILE_BYTES, MAX_REPLY_BYTES, MaterialCount, PluginError, SceneBlock,
    SceneChunk, ViewerScene,
};
use std::io::{Read, Write};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
mod composer_worker;
mod image_layout_worker;
mod provider_worker;
mod report_worker;
mod tool_form_worker;
wasmtime::component::bindgen!({ path:"../../contracts",world:"viewer-plugin" });
fn read_frame(input: &mut impl Read, maximum: usize) -> Result<Vec<u8>, PluginError> {
    let mut size = [0; 4];
    input
        .read_exact(&mut size)
        .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Missing frame length"))?;
    let size = u32::from_le_bytes(size) as usize;
    if size > maximum {
        return Err(PluginError::new(
            "PLUGIN_LIMIT",
            "Input frame exceeds budget",
        ));
    }
    let mut bytes = vec![0; size];
    input
        .read_exact(&mut bytes)
        .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Truncated input frame"))?;
    Ok(bytes)
}
fn send<T: serde::Serialize>(result: &Result<T, PluginError>) -> Result<(), PluginError> {
    let bytes = serde_json::to_vec(result)
        .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Could not encode reply"))?;
    if bytes.len() > MAX_REPLY_BYTES {
        return Err(PluginError::new(
            "PLUGIN_LIMIT",
            "Viewer reply exceeds budget",
        ));
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(&(bytes.len() as u32).to_le_bytes())
        .and_then(|()| output.write_all(&bytes))
        .and_then(|()| output.flush())
        .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Reply pipe closed"))
}
fn execute() -> Result<(), PluginError> {
    let mut input = std::io::stdin().lock();
    let component = read_frame(&mut input, MAX_COMPONENT_BYTES)?;
    let data = read_frame(&mut input, MAX_FILE_BYTES)?;
    if !component.starts_with(b"\0asm\x0d\0\x01\0") {
        return Err(PluginError::new(
            "PLUGIN_INVALID_PACKAGE",
            "Only raw WebAssembly Components are accepted",
        ));
    }
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true);
    let engine = Engine::new(&config)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not initialize Wasmtime"))?;
    // Never deserialize untrusted compiled code. No WASI or host imports.
    let component = Component::new(&engine, &component).map_err(|_| {
        PluginError::new("PLUGIN_INVALID_COMPONENT", "Component compilation failed")
    })?;
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
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not apply fuel budget"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let linker = Linker::new(&engine);
    let bindings = ViewerPlugin::instantiate(&mut store, &component, &linker).map_err(|_| {
        PluginError::new(
            "PLUGIN_PERMISSION_DENIED",
            "Component requires unsupported imports or could not initialize",
        )
    })?;
    let scene = bindings
        .launcher_extensions_viewer()
        .call_inspect(&mut store, &data)
        .map_err(|_| {
            PluginError::new("PLUGIN_TRAP", "Viewer trapped or exceeded execution budget")
        })?
        .map_err(|e| PluginError::new(&e.code, e.message))?;
    let scene = ViewerScene {
        format_version: scene.format_version,
        width: scene.width,
        height: scene.height,
        length: scene.length,
        palette: scene.palette,
        materials: scene
            .materials
            .into_iter()
            .map(|m| MaterialCount {
                state: m.state,
                count: m.count,
            })
            .collect(),
        chunk_count: scene.chunk_count,
    };
    scene.validate()?;
    send(&Ok(scene.clone()))?;
    loop {
        let mut index = [0; 4];
        match input.read_exact(&mut index) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(_) => {
                return Err(PluginError::new(
                    "PLUGIN_PROTOCOL",
                    "Chunk request pipe closed",
                ));
            }
        }
        let index = u32::from_le_bytes(index);
        store.set_epoch_deadline(8);
        let _ = store.set_fuel(50_000_000);
        let result = if index >= scene.chunk_count {
            Err(PluginError::new(
                "VIEWER_INVALID_CHUNK",
                "Chunk is outside this scene",
            ))
        } else {
            bindings
                .launcher_extensions_viewer()
                .call_read_chunk(&mut store, index)
                .map_err(|_| PluginError::new("PLUGIN_TRAP", "Viewer chunk request trapped"))?
                .map_err(|e| PluginError::new(&e.code, e.message))
                .and_then(|chunk| {
                    let chunk = SceneChunk {
                        blocks: chunk
                            .blocks
                            .into_iter()
                            .map(|b| SceneBlock {
                                x: b.x,
                                y: b.y,
                                z: b.z,
                                palette_index: b.palette_index,
                            })
                            .collect(),
                    };
                    scene.validate_chunk(&chunk)?;
                    Ok(chunk)
                })
        };
        send(&result)?;
    }
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--tool-form-stdio") {
        if let Err(error) = tool_form_worker::execute() {
            let _ = send::<cobble_plugin_api::ToolFormOutput>(&Err(error));
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--image-layout-stdio") {
        if let Err(error) = image_layout_worker::execute() {
            let _ = send::<cobble_plugin_api::ImageSheet>(&Err(error));
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--composer-stdio") {
        if let Err(error) = composer_worker::execute() {
            let _ = send::<cobble_plugin_api::MessageDraft>(&Err(error));
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--provider-stdio") {
        if let Err(error) = provider_worker::execute() {
            let _ = send::<serde_json::Value>(&Err(error));
        }
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--report-stdio") {
        if let Err(error) = report_worker::execute() {
            let _ = send::<cobble_plugin_api::Report>(&Err(error));
        }
        return;
    }
    if std::env::args().nth(1).as_deref() != Some("--viewer-stdio") {
        std::process::exit(2);
    }
    if let Err(error) = execute() {
        let _ = send::<ViewerScene>(&Err(error));
    }
}

// SPDX-License-Identifier: MIT
//! Import-free layout guest. Native code retains all pixels and output authority.
use cobble_plugin_api::{ImageLayoutRequest, ImageOrdering, ImageRgb, ImageSheet, PluginError};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
wasmtime::component::bindgen!({path:"../../contracts/image-layout",world:"image-layout"});
pub fn execute() -> Result<(), PluginError> {
    let mut input = std::io::stdin().lock();
    let bytes = super::read_frame(&mut input, cobble_plugin_api::MAX_COMPONENT_BYTES)?;
    let request: ImageLayoutRequest = serde_json::from_slice(&super::read_frame(&mut input, 8192)?)
        .map_err(|_| {
            PluginError::new("PLUGIN_INVALID_IMAGE_LAYOUT", "Invalid image descriptors")
        })?;
    request.validate_bounds()?;
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
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not initialize layout engine"))?;
    let component = Component::new(&engine, bytes)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_COMPONENT", "Layout compilation failed"))?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(32 * 1024 * 1024)
        .table_elements(10000)
        .instances(16)
        .memories(1)
        .tables(16)
        .trap_on_grow_failure(true)
        .build();
    let mut store: Store<StoreLimits> = Store::new(&engine, limits);
    store.limiter(|state| state);
    store
        .set_fuel(100_000_000)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not apply layout fuel budget"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let linker = Linker::new(&engine);
    let bindings = ImageLayout::instantiate(&mut store, &component, &linker).map_err(|_| {
        PluginError::new(
            "PLUGIN_PERMISSION_DENIED",
            "Unsupported layout imports or initialization failure",
        )
    })?;
    use exports::launcher::image_layout::layout as wit;
    let wire = wit::Request {
        locale: request.locale.clone(),
        columns: request.columns,
        ordering: match request.ordering {
            ImageOrdering::Selected => wit::Ordering::Selected,
            ImageOrdering::Oldest => wit::Ordering::Oldest,
            ImageOrdering::Newest => wit::Ordering::Newest,
        },
        images: request
            .images
            .iter()
            .map(|image| wit::Image {
                id: image.id.clone(),
                name: image.name.clone(),
                width: image.width,
                height: image.height,
                modified_at: image.modified_at,
            })
            .collect(),
    };
    let result = bindings
        .launcher_image_layout_layout()
        .call_compose(&mut store, &wire)
        .map_err(|_| PluginError::new("PLUGIN_TRAP", "Layout trapped or exceeded budget"))?
        .map_err(|_| {
            PluginError::new("PLUGIN_IMAGE_LAYOUT_FAILED", "Layout could not be prepared")
        })?;
    let sheet = ImageSheet {
        columns: result.columns,
        gap: result.gap,
        background: ImageRgb {
            red: result.background.red,
            green: result.background.green,
            blue: result.background.blue,
        },
        image_ids: result.image_ids,
    };
    sheet.validate(&request)?;
    super::send(&Ok(sheet))
}

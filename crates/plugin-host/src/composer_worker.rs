// SPDX-License-Identifier: MIT
//! Import-free, one-shot message composer. No environment, filesystem or network is linked.
use cobble_plugin_api::{MessageComposerRequest, MessageDraft, PluginError};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
wasmtime::component::bindgen!({path:"../../contracts/message-composer",world:"message-composer"});
pub fn execute() -> Result<(), PluginError> {
    let mut input = std::io::stdin().lock();
    let bytes = super::read_frame(&mut input, cobble_plugin_api::MAX_COMPONENT_BYTES)?;
    let request: MessageComposerRequest =
        serde_json::from_slice(&super::read_frame(&mut input, 8192)?)
            .map_err(|_| PluginError::new("PLUGIN_INVALID_CONTEXT", "Invalid composer request"))?;
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
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not initialize composer engine"))?;
    let component = Component::new(&engine, bytes)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_COMPONENT", "Composer compilation failed"))?;
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
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not apply composer fuel budget"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        for _ in 0..10 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let linker = Linker::new(&engine);
    let bindings = MessageComposer::instantiate(&mut store, &component, &linker).map_err(|_| {
        PluginError::new(
            "PLUGIN_PERMISSION_DENIED",
            "Unsupported composer imports or initialization failure",
        )
    })?;
    use exports::launcher::message_composer::composer as wit;
    let identity = |value: cobble_plugin_api::PublicIdentity| wit::PublicIdentity {
        id: value.id,
        display_name: value.display_name,
    };
    let request = wit::Request {
        note: request.note,
        context: wit::Context {
            locale: request.context.locale,
            public_self: request.context.public_self.map(identity),
            selected_peer: request.context.selected_peer.map(|peer| wit::SelectedPeer {
                identity: identity(peer.identity),
                source: match peer.source {
                    cobble_plugin_api::PeerSource::Contact => wit::PeerSource::Contact,
                    cobble_plugin_api::PeerSource::Discovery => wit::PeerSource::Discovery,
                    cobble_plugin_api::PeerSource::Session => wit::PeerSource::Session,
                },
            }),
            selected_session: request.context.selected_session.map(|session| {
                wit::SelectedSession {
                    game_name: session.game_name,
                    game_version: session.game_version,
                    loader: session.loader,
                }
            }),
        },
    };
    let result = bindings
        .launcher_message_composer_composer()
        .call_compose(&mut store, &request)
        .map_err(|_| PluginError::new("PLUGIN_TRAP", "Composer trapped or exceeded budget"))?
        .map_err(|_| {
            PluginError::new("PLUGIN_COMPOSE_FAILED", "Composer could not prepare draft")
        })?;
    let draft = MessageDraft { text: result.text };
    draft.validate()?;
    super::send(&Ok(draft))
}

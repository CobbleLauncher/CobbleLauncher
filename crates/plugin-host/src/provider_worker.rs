// SPDX-License-Identifier: MIT
//! Pure Provider Component. No WASI, network, files, credentials or host imports.
use cobble_plugin_api::PluginError;
use cobble_plugin_host::provider::{self as dto, ProviderOperation, ProviderRequest};
use serde::Deserialize;
use std::time::{Duration, Instant};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};
wasmtime::component::bindgen!({path:"../../contracts/provider",world:"blueprint-provider"});
use exports::launcher::blueprint_provider::provider as wit;
#[derive(Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
enum Job {
    Plan {
        request: ProviderRequest,
    },
    Normalize {
        request: ProviderRequest,
        statuses: Vec<u16>,
    },
}
fn error(code: &str, message: &str) -> PluginError {
    PluginError::new(code, message)
}
fn request(value: &ProviderRequest) -> wit::Request {
    wit::Request {
        operation: match value.operation {
            ProviderOperation::Search => wit::Operation::Search,
            ProviderOperation::Detail => wit::Operation::Detail,
            ProviderOperation::Download => wit::Operation::Download,
        },
        search: value.search.as_ref().map(|q| wit::SearchQuery {
            java_release: q.java_release.clone(),
            text: q.text.clone(),
            format: q.format.clone(),
            mod_namespace: q.mod_namespace.clone(),
            cursor: q.cursor.clone(),
            limit: q.limit,
        }),
        key: value.key.as_ref().map(|key| wit::ResourceKey {
            id: key.id.clone(),
            version: key.version,
        }),
    }
}
fn resource(value: wit::Resource) -> dto::ProviderResource {
    dto::ProviderResource {
        id: value.id,
        title: value.title,
        description: value.description,
        version: value.version,
        format: value.format,
        author: value.author,
        attribution: value.attribution,
        license: value.license,
        java_release: value.java_release,
        namespaces: value.namespaces,
        preview_url: value.preview_url,
    }
}
fn failure(value: wit::Failure) -> PluginError {
    if value.code.is_empty() || value.code.len() > 80 || value.message.len() > 2048 {
        return error("PLUGIN_INVALID_PROVIDER", "Provider error exceeds budget");
    }
    error(&value.code, &value.message)
}
pub(crate) fn execute() -> Result<(), PluginError> {
    let _entered = Instant::now();
    let mut input = std::io::stdin().lock();
    let bytes = super::read_frame(&mut input, cobble_plugin_api::MAX_COMPONENT_BYTES)?;
    drop(input);
    if !bytes.starts_with(b"\0asm\x0d\0\x01\0") {
        return Err(error(
            "PLUGIN_INVALID_PACKAGE",
            "Expected raw Provider Component",
        ));
    }
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true)
        .cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::new(&config)
        .map_err(|_| error("PLUGIN_ENGINE", "Could not initialize Provider engine"))?;
    let _engine_ready = Instant::now();
    let component = Component::new(&engine, &bytes)
        .map_err(|_| error("PLUGIN_INVALID_COMPONENT", "Provider compilation failed"))?;
    let _compiled = Instant::now();
    let limits = StoreLimitsBuilder::new()
        .memory_size(64 * 1024 * 1024)
        .table_elements(10000)
        .instances(16)
        .memories(1)
        .tables(16)
        .trap_on_grow_failure(true)
        .build();
    let mut store = Store::new(&engine, limits);
    store.limiter(|limits| limits);
    store
        .set_fuel(400_000_000)
        .map_err(|_| error("PLUGIN_ENGINE", "Could not apply Provider fuel"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        for _ in 0..310 {
            std::thread::sleep(Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let linker = Linker::new(&engine);
    let bindings =
        BlueprintProvider::instantiate(&mut store, &component, &linker).map_err(|_| {
            error(
                "PLUGIN_PERMISSION_DENIED",
                "Provider imports or initialization denied",
            )
        })?;
    let _instantiated = Instant::now();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let result = read_job(&mut input);
            let stop = !matches!(result, Ok(Some(_)));
            if send.send(result).is_err() || stop {
                break;
            }
        }
    });
    super::send(&Ok(serde_json::json!({"kind":"ready"})))?;
    for _ in 0..128 {
        let Some(remaining) = Duration::from_secs(300).checked_sub(_entered.elapsed()) else {
            return Ok(());
        };
        let (job, responses) = match receive.recv_timeout(remaining.min(Duration::from_secs(60))) {
            Ok(Ok(Some(value))) => value,
            Ok(Ok(None)) | Err(_) => return Ok(()),
            Ok(Err(error)) => return Err(error),
        };
        store
            .set_fuel(400_000_000)
            .map_err(|_| error("PLUGIN_ENGINE", "Could not reset Provider fuel"))?;
        store.set_epoch_deadline(8);
        let _call_started = Instant::now();
        let request = request(match &job {
            Job::Plan { request } | Job::Normalize { request, .. } => request,
        });
        let value = match job {
            Job::Plan { .. } => {
                let plan = bindings
                    .launcher_blueprint_provider_provider()
                    .call_plan(&mut store, &request)
                    .map_err(|_| {
                        error(
                            "PLUGIN_TRAP",
                            "Provider planning trapped or exceeded budget",
                        )
                    })?
                    .map_err(failure)?;
                let plan: Vec<_> = plan
                    .into_iter()
                    .map(|r| dto::GetRequest {
                        url: r.url,
                        maximum_bytes: r.maximum_bytes,
                    })
                    .collect();
                serde_json::json!({"kind":"plan","value":plan})
            }
            Job::Normalize { .. } => {
                let value = bindings
                    .launcher_blueprint_provider_provider()
                    .call_normalize(&mut store, &request, &responses)
                    .map_err(|_| {
                        error(
                            "PLUGIN_TRAP",
                            "Provider normalization trapped or exceeded budget",
                        )
                    })?
                    .map_err(failure)?;
                let output = dto::ProviderOutput {
                    items: value.items.into_iter().map(resource).collect(),
                    next_cursor: value.next_cursor,
                    detail: value.detail.map(resource),
                    download: value.download.map(|v| dto::FileProposal {
                        url: v.url,
                        suggested_filename: v.suggested_filename,
                        original_filename: v.original_filename,
                        declared_bytes: v.declared_bytes,
                        format: v.format,
                        id: v.id,
                        version: v.version,
                        title: v.title,
                        author: v.author,
                        attribution: v.attribution,
                        license: v.license,
                        sha256: v.sha256,
                    }),
                };
                serde_json::json!({"kind":"normalized","value":output})
            }
        };
        #[cfg(debug_assertions)]
        if std::env::var_os("COBBLE_PROVIDER_PROFILE").is_some() {
            eprintln!(
                "provider-profile compile_ms={} startup_ms={} export_ms={}",
                _compiled.duration_since(_engine_ready).as_millis(),
                _instantiated.duration_since(_entered).as_millis(),
                _call_started.elapsed().as_millis()
            );
        }
        super::send(&Ok(value))?;
    }
    Ok(())
}

fn read_job(
    input: &mut impl std::io::Read,
) -> Result<Option<(Job, Vec<wit::GetResponse>)>, PluginError> {
    let mut first = [0; 1];
    if input
        .read(&mut first)
        .map_err(|_| error("PLUGIN_PROTOCOL", "Provider input failed"))?
        == 0
    {
        return Ok(None);
    }
    let mut length = [0; 4];
    length[0] = first[0];
    input
        .read_exact(&mut length[1..])
        .map_err(|_| error("PLUGIN_PROTOCOL", "Truncated Provider request"))?;
    let count = u32::from_le_bytes(length) as usize;
    if count > 16 * 1024 {
        return Err(error("PLUGIN_LIMIT", "Provider request exceeds budget"));
    }
    let mut bytes = vec![0; count];
    input
        .read_exact(&mut bytes)
        .map_err(|_| error("PLUGIN_PROTOCOL", "Truncated Provider request"))?;
    let job: Job = serde_json::from_slice(&bytes)
        .map_err(|_| error("PLUGIN_PROTOCOL", "Invalid Provider request"))?;
    let (request, statuses) = match &job {
        Job::Plan { request } => (request, None),
        Job::Normalize { request, statuses } => (request, Some(statuses)),
    };
    request.validate()?;
    let mut responses = vec![];
    if let Some(statuses) = statuses {
        if statuses.is_empty() || statuses.len() > 2 {
            return Err(error(
                "PLUGIN_LIMIT",
                "Provider response count exceeds budget",
            ));
        }
        for status in statuses {
            responses.push(wit::GetResponse {
                status: *status,
                body: super::read_frame(input, dto::MAX_METADATA_BYTES)?,
            });
        }
    }
    Ok(Some((job, responses)))
}

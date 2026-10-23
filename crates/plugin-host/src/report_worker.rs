// SPDX-License-Identifier: MIT
use cobble_plugin_api::{PluginError, Report};
use std::time::{Duration, Instant};
use wasmtime::component::{Component, Linker, Resource, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
wasmtime::component::bindgen!({path:"../../contracts/report",world:"report-plugin",with:{"launcher:resource-report/resources.input": Snapshot}});
mod context_bindings {
    wasmtime::component::bindgen!({path:"../../contracts/report/deps/context",world:"context-import"});
}
use launcher::resource_report::resources::{Failure, Metadata};

#[derive(Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WireContext {
    locale: String,
    settings: std::collections::BTreeMap<String, serde_json::Value>,
}
pub struct Snapshot {
    bytes: Vec<u8>,
    media: String,
    digest: String,
    read: u64,
}
struct State {
    table: ResourceTable,
    limits: StoreLimits,
    deadline: Instant,
    live: bool,
    context: Option<WireContext>,
}
impl State {
    fn active(&self) -> Result<(), Failure> {
        if !self.live {
            return Err(Failure::Revoked);
        }
        if Instant::now() >= self.deadline {
            return Err(Failure::Cancelled);
        }
        Ok(())
    }
}
impl launcher::resource_report::resources::Host for State {
    fn cancelled(&mut self) -> bool {
        self.active().is_err()
    }
}
impl launcher::resource_report::resources::HostInput for State {
    fn describe(&mut self, input: Resource<Snapshot>) -> Result<Metadata, Failure> {
        self.active()?;
        let input = self.table.get(&input).map_err(|_| Failure::Revoked)?;
        Ok(Metadata {
            media_type: input.media.clone(),
            byte_length: input.bytes.len() as u64,
            sha256: input.digest.clone(),
        })
    }
    fn read_at(
        &mut self,
        input: Resource<Snapshot>,
        offset: u64,
        maximum_bytes: u32,
    ) -> Result<Vec<u8>, Failure> {
        self.active()?;
        if !(1..=65536).contains(&maximum_bytes) {
            return Err(Failure::InvalidRange);
        }
        let input = self.table.get_mut(&input).map_err(|_| Failure::Revoked)?;
        let length = input.bytes.len() as u64;
        if offset > length || offset.checked_add(u64::from(maximum_bytes)).is_none() {
            return Err(Failure::InvalidRange);
        }
        let count = u64::from(maximum_bytes).min(length - offset);
        let read = input.read.checked_add(count).ok_or(Failure::Limit)?;
        if read > length.saturating_mul(4).min(64 * 1024 * 1024) {
            return Err(Failure::Limit);
        }
        input.read = read;
        Ok(input.bytes[offset as usize..(offset + count) as usize].to_vec())
    }
    fn drop(&mut self, input: Resource<Snapshot>) -> wasmtime::Result<()> {
        self.table.delete(input)?;
        Ok(())
    }
}
impl context_bindings::launcher::report_context::context::Host for State {
    fn get(&mut self) -> context_bindings::launcher::report_context::context::Snapshot {
        let context = self
            .context
            .as_ref()
            .expect("context import only linked for declared invocation");
        context_bindings::launcher::report_context::context::Snapshot {
            locale: context.locale.clone(),
            settings_json: serde_json::to_string(&context.settings).expect("bounded context map"),
        }
    }
}
pub(crate) fn execute() -> Result<(), PluginError> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut stdin = std::io::stdin().lock();
    let bytes = super::read_frame(&mut stdin, cobble_plugin_api::MAX_COMPONENT_BYTES)?;
    let data = super::read_frame(&mut stdin, cobble_plugin_api::MAX_FILE_BYTES)?;
    let media = String::from_utf8(super::read_frame(&mut stdin, 80)?)
        .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Invalid media type"))?;
    // Legacy senders close stdin after three frames. A fourth bounded frame
    // selects the explicitly declared context ABI without changing legacy bytes.
    let mut first = [0u8; 1];
    let context =
        if std::io::Read::read(&mut stdin, &mut first)
            .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Invalid context frame"))?
            == 0
        {
            None
        } else {
            let mut length = [0u8; 4];
            length[0] = first[0];
            std::io::Read::read_exact(&mut stdin, &mut length[1..])
                .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Truncated context frame"))?;
            let count = u32::from_le_bytes(length) as usize;
            if count > 16384 {
                return Err(PluginError::new("PLUGIN_LIMIT", "Context exceeds budget"));
            }
            let mut bytes = vec![0; count];
            std::io::Read::read_exact(&mut stdin, &mut bytes)
                .map_err(|_| PluginError::new("PLUGIN_PROTOCOL", "Truncated context frame"))?;
            Some(serde_json::from_slice::<WireContext>(&bytes).map_err(|_| {
                PluginError::new("PLUGIN_INVALID_CONTEXT", "Invalid report context")
            })?)
        };
    if context
        .as_ref()
        .is_some_and(|context| !matches!(context.locale.as_str(), "en" | "zh-CN"))
    {
        return Err(PluginError::new(
            "PLUGIN_INVALID_CONTEXT",
            "Unsupported report locale",
        ));
    }
    cobble_plugin_host::validate_report_input(&bytes, &data, &media)?;
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
        .epoch_interruption(true);
    // Reports are short-lived: avoid optimizing every function on each open.
    config.cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::new(&config)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not initialize report engine"))?;
    let component = Component::new(&engine, &bytes)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_COMPONENT", "Report compilation failed"))?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(64 * 1024 * 1024)
        .table_elements(10000)
        .instances(16)
        .memories(1)
        .tables(16)
        .trap_on_grow_failure(true)
        .build();
    let mut state = State {
        table: ResourceTable::new(),
        limits,
        deadline,
        live: true,
        context: context.clone(),
    };
    let input = state
        .table
        .push(Snapshot {
            digest: format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&data)),
            bytes: data.clone(),
            media: media.clone(),
            read: 0,
        })
        .map_err(|_| PluginError::new("PLUGIN_LIMIT", "Resource table full"))?;
    let mut store = Store::new(&engine, state);
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(400_000_000)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not apply report fuel budget"))?;
    store.set_epoch_deadline(8);
    let tick = engine.clone();
    std::thread::spawn(move || {
        for _ in 0..10 {
            std::thread::sleep(Duration::from_secs(1));
            tick.increment_epoch();
        }
    });
    let mut linker = Linker::new(&engine);
    ReportPlugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not bind report resources"))?;
    if context.is_some() {
        context_bindings::ContextImport::add_to_linker::<_, wasmtime::component::HasSelf<_>>(
            &mut linker,
            |state| state,
        )
        .map_err(|_| PluginError::new("PLUGIN_ENGINE", "Could not bind report context"))?;
    }
    let bindings = ReportPlugin::instantiate(&mut store, &component, &linker).map_err(|_| {
        PluginError::new(
            "PLUGIN_PERMISSION_DENIED",
            "Unsupported report imports or initialization failure",
        )
    })?;
    let result = bindings
        .launcher_resource_report_reports()
        .call_analyze(&mut store, Resource::new_borrow(input.rep()));
    store.data_mut().live = false;
    let _ = store.data_mut().table.delete(input);
    let report = result
        .map_err(|_| PluginError::new("PLUGIN_TRAP", "Report trapped or exceeded budget"))?
        .map_err(|_| PluginError::new("PLUGIN_REPORT_FAILED", "Report analysis failed"))?;
    let report = Report {
        schema_version: report.schema_version,
        findings: report
            .findings
            .into_iter()
            .map(|finding| cobble_plugin_api::report::Finding {
                code: finding.code,
                severity: match finding.severity {
                    exports::launcher::resource_report::reports::Severity::Info => {
                        cobble_plugin_api::report::Severity::Info
                    }
                    exports::launcher::resource_report::reports::Severity::Warning => {
                        cobble_plugin_api::report::Severity::Warning
                    }
                    exports::launcher::resource_report::reports::Severity::Error => {
                        cobble_plugin_api::report::Severity::Error
                    }
                },
                title: finding.title,
                message: finding.message,
                evidence: finding
                    .evidence
                    .into_iter()
                    .map(|e| cobble_plugin_api::report::Evidence {
                        input_index: e.input_index,
                        first_line: e.first_line,
                        last_line: e.last_line,
                    })
                    .collect(),
                suggestions: finding.suggestions,
            })
            .collect(),
        tables: report
            .tables
            .into_iter()
            .map(|table| cobble_plugin_api::report::Table {
                columns: table.columns,
                rows: table.rows,
            })
            .collect(),
    };
    cobble_plugin_api::report::validate_report(&report, &media, &data)?;
    super::send(&Ok(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher::resource_report::resources::HostInput;
    #[test]
    fn resource_ranges_budgets_and_revocation() {
        let mut state = State {
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().build(),
            deadline: Instant::now() + Duration::from_secs(1),
            live: true,
            context: None,
        };
        let input = state
            .table
            .push(Snapshot {
                bytes: vec![1, 2, 3],
                media: "text/plain".into(),
                digest: "digest".into(),
                read: 0,
            })
            .unwrap();
        let borrowed = || Resource::new_borrow(input.rep());
        assert!(matches!(
            state.read_at(borrowed(), 0, 0),
            Err(Failure::InvalidRange)
        ));
        assert!(matches!(
            state.read_at(borrowed(), u64::MAX, 1),
            Err(Failure::InvalidRange)
        ));
        assert!(matches!(
            state.read_at(borrowed(), 4, 1),
            Err(Failure::InvalidRange)
        ));
        assert!(state.read_at(borrowed(), 3, 65536).unwrap().is_empty());
        for _ in 0..4 {
            assert_eq!(state.read_at(borrowed(), 0, 3).unwrap(), vec![1, 2, 3]);
        }
        assert!(matches!(
            state.read_at(borrowed(), 0, 1),
            Err(Failure::Limit)
        ));
        state.live = false;
        assert!(matches!(state.describe(borrowed()), Err(Failure::Revoked)));
        state.live = true;
        state.deadline = Instant::now();
        assert!(matches!(
            state.describe(borrowed()),
            Err(Failure::Cancelled)
        ));
        state.deadline = Instant::now() + Duration::from_secs(1);
        let rep = input.rep();
        state.table.delete(input).unwrap();
        assert!(matches!(
            state.describe(Resource::new_borrow(rep)),
            Err(Failure::Revoked)
        ));
    }
}

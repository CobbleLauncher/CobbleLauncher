// SPDX-License-Identifier: MIT
//! Import-free form computation in a bounded, owned worker.
use cobble_plugin_api::{
    MAX_COMPONENT_BYTES, PluginError, PluginManifest, ToolFormOutput, ToolFormRequest,
};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn run_tool_form(
    host: &Path,
    component: &[u8],
    manifest: &PluginManifest,
    request: &ToolFormRequest,
    cancel: Arc<AtomicBool>,
) -> Result<ToolFormOutput, PluginError> {
    request.validate(manifest)?;
    if component.len() > MAX_COMPONENT_BYTES {
        return Err(PluginError::new("PLUGIN_LIMIT", "Component exceeds budget"));
    }
    let request = serde_json::to_vec(request)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_TOOL_FORM", "Invalid tool input"))?;
    super::report::run_worker(
        host,
        "--tool-form-stdio",
        vec![component.to_vec(), request],
        cancel,
        |output: ToolFormOutput| {
            output.validate()?;
            Ok(output)
        },
    )
}

// SPDX-License-Identifier: MIT
//! Pure message drafting in a bounded owned worker. This adapter never sends messages.
use cobble_plugin_api::{
    MAX_COMPONENT_BYTES, MessageComposerRequest, MessageDraft, PluginError, PluginManifest,
};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn run_message_composer(
    host: &Path,
    component: &[u8],
    manifest: &PluginManifest,
    request: &MessageComposerRequest,
    cancel: Arc<AtomicBool>,
) -> Result<MessageDraft, PluginError> {
    request.validate(manifest)?;
    if component.len() > MAX_COMPONENT_BYTES {
        return Err(PluginError::new("PLUGIN_LIMIT", "Component exceeds budget"));
    }
    let request = serde_json::to_vec(request)
        .map_err(|_| PluginError::new("PLUGIN_INVALID_CONTEXT", "Invalid composer request"))?;
    if request.len() > 8192 {
        return Err(PluginError::new(
            "PLUGIN_LIMIT",
            "Composer context exceeds budget",
        ));
    }
    super::report::run_worker(
        host,
        "--composer-stdio",
        vec![component.to_vec(), request],
        cancel,
        |draft: MessageDraft| {
            draft.validate()?;
            Ok(draft)
        },
    )
}

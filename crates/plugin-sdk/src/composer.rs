// SPDX-License-Identifier: MIT
//! Execute an actual composer Component using a bounded explicit public fixture.
use cobble_plugin_api::MessageComposerRequest;
use std::{
    error::Error,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn test(package: &Path, host: &Path, input: &Path, json: bool) -> Result<(), Box<dyn Error>> {
    let (manifest, component) = cobble_plugin_host::read_package(package)?;
    let file = fs::File::open(input)?;
    if !file.metadata()?.is_file() {
        return Err("Composer fixture must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err("Composer fixture exceeds 8 KiB".into());
    }
    let request: MessageComposerRequest = serde_json::from_slice(&bytes)?;
    request.validate(&manifest)?;
    let draft = cobble_plugin_host::run_message_composer(
        host,
        &component,
        &manifest,
        &request,
        Arc::new(AtomicBool::new(false)),
    )?;
    if json {
        println!(
            "{}",
            serde_json::json!({"schema_version":1,"plugin_id":manifest.id,"mode":"offline-draft","draft":draft})
        );
    } else {
        println!("{}", draft.text);
    }
    Ok(())
}

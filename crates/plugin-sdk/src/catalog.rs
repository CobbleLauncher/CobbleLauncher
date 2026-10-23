// SPDX-License-Identifier: MIT
//! Local catalog authoring checks; no downloads or plugin execution.
use cobble_plugin_host::catalog::{MAX_CATALOG_BYTES, parse_index};
use std::{error::Error, fs::File, io::Read, path::Path};

pub fn check(path: &Path, json: bool) -> Result<(), Box<dyn Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_CATALOG_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let index = parse_index(&bytes)?;
    let unsupported = index
        .plugins
        .iter()
        .filter(|plugin| !plugin.supported())
        .count();
    if json {
        println!(
            "{}",
            serde_json::json!({"name": index.name, "entries": index.plugins.len(), "unsupported": unsupported})
        );
    } else {
        println!(
            "{}: {} entries, {} unsupported",
            index.name,
            index.plugins.len(),
            unsupported
        );
    }
    Ok(())
}

// SPDX-License-Identifier: MIT
//! Exercise a real layout component with explicitly supplied screenshot metadata.
use cobble_plugin_api::ImageLayoutRequest;
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
        return Err("Layout fixture must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    if bytes.len() > 8192 {
        return Err("Layout fixture exceeds 8 KiB".into());
    }
    let request: ImageLayoutRequest = serde_json::from_slice(&bytes)?;
    request.validate(&manifest)?;
    let sheet = cobble_plugin_host::run_image_layout(
        host,
        &component,
        &manifest,
        &request,
        Arc::new(AtomicBool::new(false)),
    )?;
    if json {
        println!(
            "{}",
            serde_json::json!({"schema_version":1,"plugin_id":manifest.id,"mode":"offline-layout","sheet":sheet})
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&sheet)?);
    }
    Ok(())
}

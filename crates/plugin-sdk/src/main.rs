// SPDX-License-Identifier: MIT
//! Local development tooling; does not publish packages or grant permissions.
mod catalog;
mod composer;
mod image_layout;
mod interfaces;
mod project;
mod provider;
mod report;
mod scaffold;
mod viewer;
use cobble_plugin_api::PluginManifest;
use std::{error::Error, fs, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command, options @ ..] if command == "interfaces" => interfaces::inspect(options)?,
        [command, index] if command == "catalog-check" => {
            catalog::check(Path::new(index), false)?;
        }
        [command, index, flag] if command == "catalog-check" && flag == "--json" => {
            catalog::check(Path::new(index), true)?;
        }
        [command, directory, output] if command == "build" => {
            project::build(Path::new(directory), Path::new(output))?;
        }
        [command, directory] if command == "check" => {
            let (manifest, _) = cobble_plugin_host::read_package(Path::new(directory))?;
            println!("{} {}: API {}, package metadata, raw component header and SHA-256 checked", manifest.id, manifest.version, manifest.api_version);
        }
        [command, component, manifest_path, output] if command == "pack" => {
            let bytes = fs::read(component)?;
            if !bytes.starts_with(b"\0asm\x0d\0\x01\0") || bytes.len() > cobble_plugin_api::MAX_COMPONENT_BYTES {
                return Err("Expected a raw WebAssembly Component within the size limit; use build to componentize a Rust project".into());
            }
            let mut manifest: PluginManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
            manifest.component_sha256 = cobble_plugin_host::digest(&bytes);
            manifest.validate()?;
            fs::create_dir(output)?;
            fs::write(Path::new(output).join("plugin.json"), serde_json::to_vec_pretty(&manifest)?)?;
            fs::write(Path::new(output).join("component.wasm"), bytes)?;
            cobble_plugin_host::copy_package_assets(Path::new(manifest_path).parent().unwrap_or(Path::new(".")), Path::new(output), &manifest)?;
            cobble_plugin_host::read_package(Path::new(output))?;
            println!("{output}");
        }
        [command, package, host, input] if command == "test-composer" => composer::test(Path::new(package),Path::new(host),Path::new(input),false)?,
        [command, package, host, input] if command == "test-image-layout" => image_layout::test(Path::new(package),Path::new(host),Path::new(input),false)?,
        [command, package, host, input, flag] if command == "test-image-layout" && flag=="--json" => image_layout::test(Path::new(package),Path::new(host),Path::new(input),true)?,
        [command, package, host, input, flag] if command == "test-composer" && flag=="--json" => composer::test(Path::new(package),Path::new(host),Path::new(input),true)?,
        [command, package, host, input] if command == "test" => {
            viewer::test(Path::new(package), Path::new(host), Path::new(input), false)?;
        }
        [command, package, host, input, flag] if command == "test" && flag == "--json" => {
            viewer::test(Path::new(package), Path::new(host), Path::new(input), true)?;
        }
        [command, package, host, input, options @ ..] if command == "test-report" => {
            report::test(Path::new(package), Path::new(host), Path::new(input), options)?;
        }
        [command, package, host, request, responses @ ..] if command == "test-provider" => {
            provider::test(Path::new(package),Path::new(host),Path::new(request),responses)?;
        }
        [command, id, directory, origin] if command == "new-provider" => scaffold::create_provider(id, Path::new(directory), origin)?,
        [command, id, directory] if command == "new" => scaffold::create(id, Path::new(directory), false)?,
        [command, id, directory] if command == "new-report" => scaffold::create(id, Path::new(directory), true)?,
        _ => return Err("Usage: cobble-plugin-sdk interfaces PACKAGE_DIR [--registry REGISTRY_DIR] [--json] | interfaces --installed ID --registry REGISTRY_DIR [--json] | new ID DIR | new-report ID DIR | new-provider ID DIR HTTPS_ORIGIN | build PROJECT NEW_PACKAGE_DIR | check PACKAGE_DIR | catalog-check INDEX_JSON [--json] | pack COMPONENT MANIFEST NEW_DIR | test PACKAGE_DIR HOST_EXE INPUT [--json] | test-image-layout PACKAGE HOST REQUEST_JSON [--json] | test-composer PACKAGE HOST REQUEST_JSON [--json] | test-provider PACKAGE HOST REQUEST_JSON RESPONSE_JSON [RESPONSE_JSON] [--json] | test-report PACKAGE_DIR HOST_EXE INPUT [--json] [--baseline PACK.mrpack] [--section overview|current|baseline|changes] [--search TEXT] [--offset N] [--limit N] [--detail ROW_ID --field path|before|after] [--locale en|zh-CN] [--setting KEY=JSON]".into()),
    }
    Ok(())
}

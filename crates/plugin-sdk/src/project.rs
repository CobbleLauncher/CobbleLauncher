// SPDX-License-Identifier: MIT
use cobble_plugin_api::PluginManifest;
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const TOOLS_VERSION: &str = "1.262.0";

fn execute(command: &mut Command, action: &str) -> Result {
    let status = command
        .status()
        .map_err(|error| format!("{action}: {error}"))?;
    if !status.success() {
        return Err(format!("{action} failed ({status}); see diagnostics above").into());
    }
    Ok(())
}

fn valid_version(version: &str) -> bool {
    let mut words = version.split_whitespace();
    words.next() == Some("wasm-tools") && words.next() == Some(TOOLS_VERSION)
}

/// Compile a local Rust cdylib, componentize, validate, then create a package.
pub fn build(project: &Path, output: &Path) -> Result {
    if output.exists() {
        return Err(format!(
            "Package output already exists: {}; choose a new directory",
            output.display()
        )
        .into());
    }
    let project = fs::canonicalize(project)?;
    let mut manifest: PluginManifest =
        serde_json::from_slice(&fs::read(project.join("plugin.json"))?)?;
    manifest.validate()?;
    let tools = std::env::var_os("COBBLE_WASM_TOOLS").unwrap_or_else(|| "wasm-tools".into());
    let version = Command::new(&tools).arg("--version").output().map_err(|error| format!("Cannot run wasm-tools: {error}. Install with cargo install wasm-tools --version {TOOLS_VERSION} --locked, or set COBBLE_WASM_TOOLS"))?;
    if !version.status.success() || !valid_version(&String::from_utf8_lossy(&version.stdout)) {
        return Err(format!("This SDK requires wasm-tools {TOOLS_VERSION}; install with cargo install wasm-tools --version {TOOLS_VERSION} --locked").into());
    }
    let metadata = Command::new("cargo")
        .current_dir(&project)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()?;
    if !metadata.status.success() {
        return Err(format!(
            "Cannot inspect Cargo project: {}",
            String::from_utf8_lossy(&metadata.stderr)
        )
        .into());
    }
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout)?;
    let artifact = artifact_path(&metadata, &project)?;
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&project).args([
        "build",
        "--release",
        "--target",
        "wasm32-unknown-unknown",
        "--lib",
    ]);
    if project.join("Cargo.lock").exists() {
        cargo.arg("--locked");
    }
    execute(
        &mut cargo,
        "Cargo build (install target with rustup target add wasm32-unknown-unknown if missing)",
    )?;
    let component = artifact.with_extension("component.wasm");
    execute(
        Command::new(&tools)
            .args(["component", "new"])
            .arg(&artifact)
            .arg("-o")
            .arg(&component),
        "Component creation",
    )?;
    execute(
        Command::new(&tools).arg("validate").arg(&component),
        "Component validation",
    )?;
    let bytes = fs::read(&component)?;
    if bytes.len() > cobble_plugin_api::MAX_COMPONENT_BYTES {
        return Err("Built component exceeds package size limit".into());
    }
    manifest.component_sha256 = cobble_plugin_host::digest(&bytes);
    manifest.validate()?;
    fs::create_dir(output)?;
    fs::write(
        output.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(output.join("component.wasm"), bytes)?;
    cobble_plugin_host::copy_package_assets(&project, output, &manifest)?;
    cobble_plugin_host::read_package(output)?;
    println!("{}", output.display());
    Ok(())
}

fn artifact_path(metadata: &serde_json::Value, project: &Path) -> Result<PathBuf> {
    let manifest = project.join("Cargo.toml");
    let package = metadata["packages"]
        .as_array()
        .and_then(|packages| {
            packages.iter().find(|package| {
                package["manifest_path"].as_str().is_some_and(|path| {
                    let path = Path::new(path);
                    path == manifest
                        || fs::canonicalize(path)
                            .ok()
                            .is_some_and(|path| path == manifest)
                })
            })
        })
        .ok_or("Cargo metadata does not contain the selected project's package")?;
    let target = package["targets"]
        .as_array()
        .and_then(|targets| {
            targets.iter().find(|target| {
                target["crate_types"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|kind| kind == "cdylib"))
            })
        })
        .ok_or("Plugin Cargo.toml must define [lib] crate-type = [\"cdylib\"]")?;
    let name = target["name"]
        .as_str()
        .ok_or("Cargo metadata is missing the library name")?;
    let directory = metadata["target_directory"]
        .as_str()
        .ok_or("Cargo metadata is missing target_directory")?;
    Ok(Path::new(directory)
        .join("wasm32-unknown-unknown/release")
        .join(format!("{}.wasm", name.replace('-', "_"))))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tools_version_is_exact() {
        assert!(valid_version("wasm-tools 1.262.0 (revision)\n"));
        for version in ["wasm-tools 1.262.01", "wasm-tools 1.261.0", "1.262.0"] {
            assert!(!valid_version(version));
        }
    }
    #[test]
    fn resolves_actual_library_and_target_directory() {
        let project = Path::new("plugin");
        let metadata = serde_json::json!({"packages":[{"manifest_path":project.join("Cargo.toml"),"targets":[{"name":"custom_viewer","crate_types":["cdylib"]}]}],"target_directory":"custom-target"});
        assert_eq!(
            artifact_path(&metadata, project).unwrap(),
            Path::new("custom-target/wasm32-unknown-unknown/release/custom_viewer.wasm")
        );
        let missing = serde_json::json!({"packages":[{"manifest_path":project.join("Cargo.toml"),"targets":[]}]});
        assert!(
            artifact_path(&missing, project)
                .unwrap_err()
                .to_string()
                .contains("cdylib")
        );
    }
}

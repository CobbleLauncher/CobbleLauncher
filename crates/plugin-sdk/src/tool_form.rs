// SPDX-License-Identifier: MIT
//! Standalone pure tool scaffolding and real isolated Component fixture execution.
use cobble_plugin_api::{MAX_TOOL_FORM_BYTES, PluginManifest, ToolFormRequest};
use std::{
    error::Error,
    fs,
    io::Read,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub fn create(id: &str, root: &Path) -> Result<(), Box<dyn Error>> {
    let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
        "id":id,"name":id,"version":"0.1.0","api_version":"0.1.0","publisher":"Local developer",
        "capabilities":["tool-form"],"file_extensions":[],"permissions":[],"component_sha256":"0".repeat(64),
        "tool_form":{"schema_version":1,"schema":{"type":"object","additionalProperties":false,"properties":{
            "text":{"type":"string","title":"Text","default":"Hello","maxLength":256}},"required":["text"]}},
        "contributions":{"schema_version":1,"commands":[{"id":format!("{id}.run"),"label":{"en":"Run tool"},"workflow":"tool_form","menus":[{"position":"tools"}]}]}
    }))?;
    manifest.validate()?;
    fs::create_dir(root)?;
    fs::create_dir(root.join("src"))?;
    fs::create_dir(root.join("wit"))?;
    fs::write(
        root.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        root.join("wit/tool.wit"),
        include_str!("../../../contracts/tool-form/tool.wit"),
    )?;
    fs::write(
        root.join("src/lib.rs"),
        include_str!("tool-form-template.rs.txt"),
    )?;
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"cobble-plugin-{id}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"MIT\"\n[lib]\ncrate-type = [\"cdylib\"]\n[dependencies]\nwit-bindgen = \"=0.63.0\"\nserde_json = \"1\"\n[workspace]\n"
        ),
    )?;
    fs::write(
        root.join("README.md"),
        "# Pure form tool\n\nThe host renders the declared JSON Schema and JSON Forms interface. Form values are transient and passed to the import-free Wasm component. Return bounded plain text for the user to review and copy. No file, network, game, or execution access is granted.\n\nBuild with `cobble-plugin-sdk build PROJECT NEW_PACKAGE`. Run a real fixture with `cobble-plugin-sdk test-tool-form PACKAGE HOST REQUEST_JSON --json`. A request is `{\"locale\":\"en\",\"input\":{\"text\":\"Hello\"}}`.\n",
    )?;
    fs::write(root.join(".gitignore"), "/target/\n/package/\n")?;
    println!("{}", root.display());
    Ok(())
}
pub fn test(package: &Path, host: &Path, input: &Path, json: bool) -> Result<(), Box<dyn Error>> {
    let (manifest, component) = cobble_plugin_host::read_package(package)?;
    let file = fs::File::open(input)?;
    if !file.metadata()?.is_file() {
        return Err("Tool fixture must be a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_TOOL_FORM_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_TOOL_FORM_BYTES {
        return Err("Tool fixture exceeds 16 KiB".into());
    }
    let request: ToolFormRequest = serde_json::from_slice(&bytes)?;
    let output = cobble_plugin_host::run_tool_form(
        host,
        &component,
        &manifest,
        &request,
        Arc::new(AtomicBool::new(false)),
    )?;
    if json {
        println!(
            "{}",
            serde_json::json!({"schema_version":1,"plugin_id":manifest.id,"output":output})
        );
    } else {
        println!("{}", output.title);
        for block in output.blocks {
            println!("{}", block.text);
        }
    }
    Ok(())
}

// SPDX-License-Identifier: MIT
//! Disk-only interface diagnostics; never executes or installs a package.
use cobble_plugin_host::{PluginRegistry, read_package};
use std::{
    error::Error,
    path::{Path, PathBuf},
};

pub fn inspect(args: &[String]) -> Result<(), Box<dyn Error>> {
    let mut package = None;
    let mut installed = None;
    let mut registry = None;
    let mut json = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" if !json => json = true,
            "--registry" if registry.is_none() => {
                index += 1;
                registry = Some(PathBuf::from(
                    args.get(index).ok_or("Missing registry directory")?,
                ));
            }
            "--installed" if installed.is_none() => {
                index += 1;
                installed = Some(
                    args.get(index)
                        .ok_or("Missing installed plugin ID")?
                        .clone(),
                );
            }
            value if !value.starts_with('-') && package.is_none() => {
                package = Some(PathBuf::from(value))
            }
            _ => return Err("Invalid interfaces arguments".into()),
        }
        index += 1;
    }
    let registry = registry.map(PluginRegistry::new);
    let (manifest, enabled, problem) = match (package, installed) {
        (Some(path), None) => (read_package(Path::new(&path))?.0, true, None),
        (None, Some(id)) => {
            let item = registry
                .as_ref()
                .ok_or("--installed requires --registry")?
                .list()?
                .into_iter()
                .find(|item| item.manifest.id == id)
                .ok_or("Plugin is not installed")?;
            (item.manifest, item.enabled, item.problem)
        }
        _ => return Err("Choose a package directory or --installed ID".into()),
    };
    let mut dependencies = Vec::new();
    let mut ready = enabled && problem.is_none();
    for (optional, requirement) in manifest
        .interfaces
        .requires
        .iter()
        .map(|r| (false, r))
        .chain(manifest.interfaces.optional.iter().map(|r| (true, r)))
    {
        let resolution = match &registry {
            Some(registry) => serde_json::to_value(registry.resolve_interface(requirement, None)?)?,
            None => {
                serde_json::json!({"id":requirement.id,"version":requirement.version,"implementations":[],"candidates":[]})
            }
        };
        let count = resolution["implementations"].as_array().map_or(0, Vec::len);
        let available = count > 0;
        if !optional && !available {
            ready = false;
        }
        let mut state = if count == 0 {
            "unavailable".to_string()
        } else if count == 1 {
            "selected".to_string()
        } else {
            "selection_required".to_string()
        };
        let mut code = if count == 0 && !optional {
            "PLUGIN_MISSING_INTERFACE"
        } else if count == 0 {
            "INTERFACE_OPTIONAL_UNAVAILABLE"
        } else if count > 1 {
            "INTERFACE_SELECTION_REQUIRED"
        } else {
            "INTERFACE_AVAILABLE"
        }
        .to_string();
        if let Some(registry) = &registry {
            match registry.resolve_manifest_interface(&manifest, &requirement.id, None, None) {
                Ok(consumer) => {
                    state = serde_json::to_value(consumer.state)?
                        .as_str()
                        .ok_or("Invalid consumer state")?
                        .to_string()
                }
                Err(error) => code = error.code,
            }
        }
        dependencies.push(serde_json::json!({"optional":optional,"available":available,"code":code,"state":state,"resolution":resolution}));
    }
    let diagnostic = serde_json::json!({"schema_version":1,"plugin_id":manifest.id,"enabled":enabled,"problem":problem,"provides":manifest.provided_interfaces(),"ready":ready,"dependencies":dependencies});
    if json {
        println!("{}", serde_json::to_string(&diagnostic)?);
    } else {
        println!(
            "{}: {}",
            manifest.id,
            if ready { "ready" } else { "unavailable" }
        );
        for dependency in dependencies {
            println!(
                "{} {} {}: {}",
                if dependency["optional"] == true {
                    "optional"
                } else {
                    "required"
                },
                dependency["resolution"]["id"].as_str().unwrap_or_default(),
                dependency["resolution"]["version"]
                    .as_str()
                    .unwrap_or_default(),
                dependency["state"].as_str().unwrap_or_default()
            );
        }
    }
    Ok(())
}

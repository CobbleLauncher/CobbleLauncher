// SPDX-License-Identifier: MIT
use cobble_plugin_api::{
    API_VERSION, CommandContribution, ContextCondition, Contributions, LocalizedText,
    MenuContribution, MenuPosition, PermissionNode, PluginManifest, REPORT_API_VERSION, Workflow,
};
use std::{error::Error, fs, path::Path};

pub fn create(id: &str, root: &Path, report: bool) -> Result<(), Box<dyn Error>> {
    let manifest = PluginManifest {
        frontend: None,
        permissions: Some(vec![PermissionNode::SelectedFileRead {
            extensions: vec![if report { "txt" } else { "schem" }.into()],
        }]),
        description: None,
        tags: Vec::new(),
        icon: None,
        id: id.into(),
        name: id.into(),
        version: "0.1.0".into(),
        api_version: if report {
            REPORT_API_VERSION
        } else {
            API_VERSION
        }
        .into(),
        publisher: "Local developer".into(),
        capabilities: vec![if report { "report" } else { "viewer" }.into()],
        composer_permissions: Default::default(),
        interfaces: cobble_plugin_api::Interfaces {
            provides: vec![cobble_plugin_api::ProvidedInterface {
                id: if report {
                    "launcher:report"
                } else {
                    "launcher:viewer"
                }
                .into(),
                version: if report {
                    REPORT_API_VERSION
                } else {
                    API_VERSION
                }
                .into(),
            }],
            ..Default::default()
        },
        file_extensions: vec![if report { "txt" } else { "schem" }.into()],
        report_protocols: vec![],
        contributions: Some(default_contributions(id, report)),
        provider_origins: vec![],
        component_sha256: "0".repeat(64),
    };
    manifest.validate()?;
    fs::create_dir(root)?;
    fs::create_dir(root.join("src"))?;
    fs::create_dir(root.join("wit"))?;
    fs::write(
        root.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        root.join("wit/plugin.wit"),
        if report {
            include_str!("../../../contracts/report/report.wit")
        } else {
            include_str!("../../../contracts/plugin.wit")
        },
    )?;
    fs::write(root.join("Cargo.toml"), cargo_manifest(id))?;
    fs::write(
        root.join("src/lib.rs"),
        if report {
            include_str!("report-template.rs.txt")
        } else {
            include_str!("template.rs.txt")
        },
    )?;
    fs::write(
        root.join("README.md"),
        if report {
            include_str!("report-readme.txt")
        } else {
            include_str!("project-readme.txt")
        },
    )?;
    fs::write(root.join(".gitignore"), "/target/\n/package/\n")?;
    println!("{}", root.display());
    Ok(())
}
fn default_contributions(id: &str, report: bool) -> Contributions {
    Contributions {
        schema_version: 1,
        commands: vec![CommandContribution {
            id: format!("{id}.open"),
            label: LocalizedText {
                en: if report {
                    "Analyze file"
                } else {
                    "Open blueprint"
                }
                .into(),
                zh_cn: None,
            },
            workflow: if report {
                Workflow::Report
            } else {
                Workflow::Viewer
            },
            menus: vec![MenuContribution {
                position: MenuPosition::Tools,
                when: ContextCondition::Always,
            }],
        }],
        settings: None,
    }
}

fn cargo_manifest(id: &str) -> String {
    // A valid plugin ID can begin with digits or be a Rust keyword.
    format!(
        "[package]\nname = \"cobble-plugin-{id}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nlicense = \"MIT\"\n[lib]\ncrate-type = [\"cdylib\"]\n[dependencies]\nwit-bindgen = \"=0.63.0\"\n[workspace]\n"
    )
}

pub fn create_provider(id: &str, root: &Path, origin: &str) -> Result<(), Box<dyn Error>> {
    let manifest = PluginManifest {
        frontend: None,
        permissions: Some(vec![PermissionNode::ProviderGet {
            origins: vec![origin.into()],
        }]),
        description: Some(cobble_plugin_api::ManifestDescription::Localized(
            LocalizedText {
                en: "Browse a public blueprint catalog".into(),
                zh_cn: Some("浏览公开蓝图目录".into()),
            },
        )),
        tags: vec!["blueprints".into(), "catalog".into()],
        icon: Some(cobble_plugin_api::PluginIcon {
            file: "icon.png".into(),
            sha256: cobble_plugin_host::digest(include_bytes!("provider-icon.png")),
        }),
        id: id.into(),
        name: id.into(),
        version: "0.1.0".into(),
        api_version: cobble_plugin_api::PROVIDER_API_VERSION.into(),
        publisher: "Local developer".into(),
        capabilities: vec!["blueprint-provider".into()],
        composer_permissions: Default::default(),
        interfaces: cobble_plugin_api::Interfaces {
            provides: vec![cobble_plugin_api::ProvidedInterface {
                id: cobble_plugin_api::BLUEPRINT_PROVIDER_INTERFACE.into(),
                version: cobble_plugin_api::PROVIDER_API_VERSION.into(),
            }],
            optional: vec![cobble_plugin_api::InterfaceRequirement::viewer()],
            ..Default::default()
        },
        file_extensions: vec![],
        report_protocols: vec![],
        contributions: None,
        provider_origins: vec![origin.into()],
        component_sha256: "0".repeat(64),
    };
    manifest.validate()?;
    fs::create_dir(root)?;
    fs::write(root.join("icon.png"), include_bytes!("provider-icon.png"))?;
    for directory in ["src", "wit", "fixtures"] {
        fs::create_dir(root.join(directory))?;
    }
    fs::write(
        root.join("plugin.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(
        root.join("wit/provider.wit"),
        include_str!("../../../contracts/provider/provider.wit"),
    )?;
    let cargo = cargo_manifest(id).replace("[workspace]", "serde = { version = \"=1.0.229\", features = [\"derive\"] }\nserde_json = \"=1.0.154\"\n[workspace]");
    fs::write(root.join("Cargo.toml"), cargo)?;
    fs::write(
        root.join("rust-toolchain.toml"),
        "[toolchain]\nchannel = \"1.99.0\"\nprofile = \"minimal\"\ntargets = [\"wasm32-unknown-unknown\"]\n",
    )?;
    fs::write(
        root.join("src/lib.rs"),
        include_str!("provider-template.rs.txt")
            .replace("__PROVIDER_ORIGIN__", &serde_json::to_string(origin)?),
    )?;
    fs::write(root.join("README.md"), include_str!("provider-readme.txt"))?;
    fs::write(root.join(".gitignore"), "/target/\n/package/\n")?;
    fs::write(root.join("fixtures/catalog.json"), br#"{"items":[{"id":"sample-hut","title":"Sample hut","version":1,"author":"Example author","bytes":16}]}"#)?;
    for (name, value) in [
        (
            "search",
            serde_json::json!({"operation":"search","search":{"java_release":"1.21.1","text":"hut","format":null,"mod_namespace":null,"cursor":null,"limit":20},"key":null}),
        ),
        (
            "detail",
            serde_json::json!({"operation":"detail","search":null,"key":{"id":"sample-hut","version":1}}),
        ),
        (
            "download",
            serde_json::json!({"operation":"download","search":null,"key":{"id":"sample-hut","version":1}}),
        ),
    ] {
        fs::write(
            root.join(format!("fixtures/{name}.json")),
            serde_json::to_vec_pretty(&value)?,
        )?;
    }
    println!("{}", root.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_manifests_declare_exact_supported_permissions() {
        struct Temporary(std::path::PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let temporary = Temporary(std::env::temp_dir().join(format!(
                "cobble-scaffold-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&temporary.0).unwrap();
        for (id, report) in [("original-viewer", false), ("original-report", true)] {
            let root = temporary.0.join(id);
            super::create(id, &root, report).unwrap();
            let manifest: cobble_plugin_api::PluginManifest =
                serde_json::from_slice(&std::fs::read(root.join("plugin.json")).unwrap()).unwrap();
            manifest.validate().unwrap();
            assert_eq!(
                manifest.permissions,
                Some(vec![cobble_plugin_api::PermissionNode::SelectedFileRead {
                    extensions: vec![if report { "txt" } else { "schem" }.into()]
                }])
            );
        }
        let root = temporary.0.join("original-provider");
        super::create_provider("original-provider", &root, "https://catalog.example").unwrap();
        let manifest: cobble_plugin_api::PluginManifest =
            serde_json::from_slice(&std::fs::read(root.join("plugin.json")).unwrap()).unwrap();
        manifest.validate().unwrap();
        assert_eq!(
            manifest.permissions,
            Some(vec![cobble_plugin_api::PermissionNode::ProviderGet {
                origins: vec!["https://catalog.example".into()]
            }])
        );
    }
    #[test]
    fn generated_commands_are_namespaced_and_bind_existing_workflows() {
        for report in [false, true] {
            let contribution = super::default_contributions("example", report);
            assert_eq!(contribution.commands[0].id, "example.open");
            assert_eq!(
                contribution.commands[0].workflow,
                if report {
                    cobble_plugin_api::Workflow::Report
                } else {
                    cobble_plugin_api::Workflow::Viewer
                }
            );
            assert!(contribution.settings.is_none());
        }
    }
    #[test]
    fn cargo_names_prefix_numeric_and_keyword_ids() {
        for id in ["123", "mod", "---"] {
            assert!(super::cargo_manifest(id).contains(&format!("name = \"cobble-plugin-{id}\"")));
        }
    }
}

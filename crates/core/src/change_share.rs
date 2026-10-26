// SPDX-License-Identifier: MIT
//! A small, source-qualified change list. It contains only selected references
//! and config text; provider files are acquired by the ordinary resolver.
use crate::environment_install::{SharedConfig, validate_shared_configs};
use crate::play_manifest::Preview;
use cobble_content::{EnvironmentSelection, EnvironmentSource, ModrinthPlan};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Read, path::Path};

type Error = Box<dyn std::error::Error + Send + Sync>;
pub const MAX_DOCUMENT_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModReference {
    pub source: EnvironmentSource,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub sha1: String,
    pub size: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Document {
    pub schema_version: u8,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: String,
    pub mods: Vec<ModReference>,
    pub configs: Vec<SharedConfig>,
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-+".contains(&byte))
}

pub fn validate(document: &Document) -> Result<(), Error> {
    if document.schema_version != 1
        || document.name.is_empty()
        || document.name.chars().count() > 80
        || document.name.chars().any(char::is_control)
        || !identifier(&document.game_version)
        || !matches!(document.loader.as_str(), "fabric" | "forge" | "neoforge")
        || !identifier(&document.loader_version)
        || document.mods.len() > 128
        || document.configs.len() > 64
        || (document.mods.is_empty() && document.configs.is_empty())
    {
        return Err("CHANGE_SHARE_DOCUMENT_INVALID".into());
    }
    let mut projects = BTreeSet::new();
    let mut names = BTreeSet::new();
    for item in &document.mods {
        if !identifier(&item.project_id)
            || !identifier(&item.version_id)
            || item.name.is_empty()
            || item.name.len() > 200
            || !item.name.ends_with(".jar")
            || item.name.contains(['/', '\\', ':'])
            || item.name.chars().any(char::is_control)
            || item.sha1.len() != 40
            || !item.sha1.bytes().all(|b| b.is_ascii_hexdigit())
            || item.size == 0
            || item.size > 512 * 1024 * 1024
            || !projects.insert((item.source, item.project_id.clone()))
            || !names.insert(item.name.to_ascii_lowercase())
        {
            return Err("CHANGE_SHARE_DOCUMENT_INVALID".into());
        }
    }
    validate_shared_configs(&document.configs).map_err(|_| "CHANGE_SHARE_CONFIG_INVALID")?;
    if document.configs.iter().any(|config| {
        !matches!(
            Path::new(&config.path).extension().and_then(|s| s.to_str()),
            Some("json" | "json5" | "toml" | "cfg" | "properties" | "yaml" | "yml" | "txt")
        )
    }) {
        return Err("CHANGE_SHARE_CONFIG_INVALID".into());
    }
    if serde_json::to_vec(document)?.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("CHANGE_SHARE_DOCUMENT_LIMIT".into());
    }
    Ok(())
}

pub fn from_preview(
    preview: &Preview,
    mod_ids: &[String],
    config_ids: &[String],
) -> Result<Document, Error> {
    let loader = preview
        .loader
        .as_deref()
        .ok_or("CHANGE_SHARE_LOADER_REQUIRED")?;
    let loader_version = preview
        .loader_version
        .as_deref()
        .ok_or("CHANGE_SHARE_LOADER_REQUIRED")?;
    if mod_ids.len() > 128 || config_ids.len() > 64 {
        return Err("CHANGE_SHARE_SELECTION_INVALID".into());
    }
    let mod_set: BTreeSet<_> = mod_ids.iter().collect();
    let config_set: BTreeSet<_> = config_ids.iter().collect();
    if mod_set.len() != mod_ids.len() || config_set.len() != config_ids.len() {
        return Err("CHANGE_SHARE_SELECTION_INVALID".into());
    }
    let mut mods = Vec::new();
    for id in mod_ids {
        let item = preview
            .mods
            .iter()
            .find(|item| &item.project_id == id)
            .ok_or("CHANGE_SHARE_SELECTION_INVALID")?;
        if item.source != "Modrinth" {
            return Err("CHANGE_SHARE_SOURCE_UNSUPPORTED".into());
        }
        mods.push(ModReference {
            source: EnvironmentSource::Modrinth,
            project_id: item.project_id.clone(),
            version_id: item.version_id.clone(),
            name: item.name.clone(),
            sha1: item.sha1.clone(),
            size: item.size,
        });
    }
    let mut configs = Vec::new();
    for id in config_ids {
        let item = preview
            .configs
            .iter()
            .find(|item| &item.id == id)
            .ok_or("CHANGE_SHARE_SELECTION_INVALID")?;
        configs.push(SharedConfig {
            path: item.name.clone(),
            text: item.text.clone(),
            sha256: format!("{:x}", Sha256::digest(item.text.as_bytes())),
        });
    }
    let document = Document {
        schema_version: 1,
        name: preview.name.clone(),
        game_version: preview.version.clone(),
        loader: loader.into(),
        loader_version: loader_version.into(),
        mods,
        configs,
    };
    validate(&document)?;
    Ok(document)
}

pub fn read(mut file: impl Read) -> Result<Document, Error> {
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("CHANGE_SHARE_DOCUMENT_LIMIT".into());
    }
    let document: Document =
        serde_json::from_slice(&bytes).map_err(|_| "CHANGE_SHARE_DOCUMENT_INVALID")?;
    validate(&document)?;
    Ok(document)
}

pub fn encode(document: &Document) -> Result<Vec<u8>, Error> {
    validate(document)?;
    Ok(serde_json::to_vec_pretty(document)?)
}

pub fn selections(document: &Document) -> Result<Vec<EnvironmentSelection>, Error> {
    validate(document)?;
    Ok(document
        .mods
        .iter()
        .map(|item| EnvironmentSelection {
            source: item.source,
            project_id: item.project_id.clone(),
            version_id: item.version_id.clone(),
        })
        .collect())
}

pub fn verify_modrinth_plan(document: &Document, plan: &ModrinthPlan) -> Result<(), Error> {
    validate(document)?;
    for expected in &document.mods {
        if expected.source != EnvironmentSource::Modrinth {
            return Err("CHANGE_SHARE_CURSEFORGE_APPLY_UNSUPPORTED".into());
        }
        let actual = plan.mods.iter().find(|item| {
            item.project_id == expected.project_id && item.version_id == expected.version_id
        });
        if !actual.is_some_and(|item| {
            item.file.sha1.eq_ignore_ascii_case(&expected.sha1)
                && item.file.size == expected.size
                && item.file.filename == expected.name
        }) {
            return Err("CHANGE_SHARE_SOURCE_CHANGED".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_only_and_private_paths_rejected() {
        let preview = Preview {
            revision: "r".into(),
            name: "Game".into(),
            version: "1.21.1".into(),
            loader: Some("fabric".into()),
            loader_version: Some("0.16.0".into()),
            mods: vec![],
            configs: vec![crate::play_manifest::ConfigChoice {
                id: "selected".into(),
                name: "config/example.json".into(),
                size: 2,
                text: "{}".into(),
            }],
            excluded_mods: vec!["private.jar".into()],
        };
        let document = from_preview(&preview, &[], &["selected".into()]).unwrap();
        let bytes = encode(&document).unwrap();
        assert!(!bytes.windows(11).any(|window| window == b"private.jar"));
        assert_eq!(read(bytes.as_slice()).unwrap().configs.len(), 1);
        let mut private = document;
        private.configs[0].path = "options.txt".into();
        assert!(validate(&private).is_err());
    }

    #[test]
    fn source_artifact_must_match_exact_shared_reference() {
        let document = Document {
            schema_version: 1,
            name: "Game".into(),
            game_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: "0.16.0".into(),
            mods: vec![ModReference {
                source: EnvironmentSource::Modrinth,
                project_id: "synthetic".into(),
                version_id: "v1".into(),
                name: "synthetic.jar".into(),
                sha1: "a".repeat(40),
                size: 10,
            }],
            configs: vec![],
        };
        let mut graph = ModrinthPlan {
            project_id: "synthetic".into(),
            version_id: "v1".into(),
            mods: vec![cobble_content::ResolvedMod {
                project_id: "synthetic".into(),
                version_id: "v1".into(),
                name: "Synthetic".into(),
                version_number: "1".into(),
                required_by: None,
                file: cobble_content::ModrinthFile {
                    filename: "synthetic.jar".into(),
                    url: "https://cdn.modrinth.com/data/synthetic/versions/v1/synthetic.jar".into(),
                    size: 10,
                    sha1: "a".repeat(40),
                    sha512: None,
                },
            }],
        };
        assert!(verify_modrinth_plan(&document, &graph).is_ok());
        graph.mods[0].file.sha1 = "b".repeat(40);
        assert_eq!(
            verify_modrinth_plan(&document, &graph)
                .unwrap_err()
                .to_string(),
            "CHANGE_SHARE_SOURCE_CHANGED"
        );
    }
}

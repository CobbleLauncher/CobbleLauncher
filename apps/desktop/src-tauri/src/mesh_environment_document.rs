// SPDX-License-Identifier: MIT
//! Share source identities and reviewed config text, never peer-provided download URLs.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModReference {
    pub source: cobble_content::EnvironmentSource,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub sha1: String,
    pub size: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub path: String,
    pub text: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema_version: u8,
    pub name: String,
    pub game_version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub mods: Vec<ModReference>,
    pub configs: Vec<Config>,
    pub excluded: Vec<String>,
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-+".contains(&byte))
}
fn digest_value(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && !value.contains(['/', '\\', ':'])
        && !value.chars().any(char::is_control)
}
pub fn validate(document: &Document) -> Result<(), String> {
    if document.schema_version != 1
        || document.name.is_empty()
        || document.name.chars().count() > 80
        || document.name.chars().any(char::is_control)
        || !identifier(&document.game_version)
        || document.mods.len() > 128
        || document.configs.len() > 64
        || document.excluded.len() > 1024
        || document.excluded.iter().any(|value| !name(value))
        || !matches!(
            document.loader.as_deref(),
            None | Some("fabric" | "forge" | "neoforge")
        )
        || document.loader.is_some() != document.loader_version.is_some()
        || document
            .loader_version
            .as_ref()
            .is_some_and(|value| !identifier(value))
    {
        return Err("MESH_ENVIRONMENT_DOCUMENT_INVALID".into());
    }
    let mut identities = BTreeSet::new();
    let mut names = BTreeSet::new();
    for item in &document.mods {
        if !identifier(&item.project_id)
            || !identifier(&item.version_id)
            || !name(&item.name)
            || !item.name.ends_with(".jar")
            || !digest_value(&item.sha1, 40)
            || item.size == 0
            || item.size > 512 * 1024 * 1024
            || !identities.insert((item.source, item.project_id.clone()))
            || !names.insert(item.name.to_ascii_lowercase())
        {
            return Err("MESH_ENVIRONMENT_DOCUMENT_INVALID".into());
        }
    }
    if !document.mods.is_empty() && document.loader.is_none() {
        return Err("MESH_ENVIRONMENT_DOCUMENT_INVALID".into());
    }
    let mut paths = BTreeSet::new();
    let mut bytes = 0;
    for item in &document.configs {
        let path = cobble_store::SafeRelativePath::parse(&item.path)
            .map_err(|_| "MESH_ENVIRONMENT_CONFIG_INVALID")?;
        bytes += item.text.len();
        if !path.as_str().starts_with("config/")
            || !matches!(
                std::path::Path::new(&item.path)
                    .extension()
                    .and_then(|value| value.to_str()),
                Some("json" | "json5" | "toml" | "cfg" | "properties" | "yaml" | "yml" | "txt")
            )
            || item.path.len() > 512
            || item.text.len() > 64 * 1024
            || item.text.contains('\0')
            || !digest_value(&item.sha256, 64)
            || cobble_plugin_host::digest(item.text.as_bytes()) != item.sha256
            || !paths.insert(item.path.to_ascii_lowercase())
            || bytes > 512 * 1024
        {
            return Err("MESH_ENVIRONMENT_CONFIG_INVALID".into());
        }
    }
    if serde_json::to_vec(document)
        .map_err(|_| "MESH_ENVIRONMENT_DOCUMENT_INVALID")?
        .len()
        > 1024 * 1024
    {
        return Err("MESH_ENVIRONMENT_DOCUMENT_LIMIT".into());
    }
    Ok(())
}
fn local_hash(root: &Path, filename: &str) -> Result<(String, u64), String> {
    if !name(filename) {
        return Err("MESH_ENVIRONMENT_LOCAL_FILE_INVALID".into());
    }
    let mods = root.join("mods");
    let base = fs::canonicalize(&mods).map_err(|_| "MESH_ENVIRONMENT_LOCAL_FILE_INVALID")?;
    let root = fs::canonicalize(root).map_err(|_| "MESH_ENVIRONMENT_LOCAL_FILE_INVALID")?;
    if base.parent() != Some(root.as_path()) {
        return Err("MESH_ENVIRONMENT_LOCAL_FILE_INVALID".into());
    }
    let path = mods.join(filename);
    let metadata =
        fs::symlink_metadata(&path).map_err(|_| "MESH_ENVIRONMENT_LOCAL_FILE_INVALID")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("MESH_ENVIRONMENT_LOCAL_FILE_INVALID".into());
        }
    }
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 512 * 1024 * 1024
    {
        return Err("MESH_ENVIRONMENT_LOCAL_FILE_INVALID".into());
    }
    let mut file = fs::File::open(path).map_err(|_| "MESH_ENVIRONMENT_LOCAL_FILE_INVALID")?;
    let mut hash = sha1::Sha1::new();
    let mut buffer = [0; 128 * 1024];
    let mut size = 0;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "MESH_ENVIRONMENT_LOCAL_FILE_INVALID")?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > 512 * 1024 * 1024 {
            return Err("MESH_ENVIRONMENT_LOCAL_FILE_INVALID".into());
        }
        hash.update(&buffer[..n]);
    }
    Ok((format!("{:x}", hash.finalize()), size))
}
pub async fn capture(
    app: &AppHandle,
    game_id: &str,
) -> Result<
    (
        Document,
        Vec<cobble_core::play_manifest::ConfigChoice>,
        String,
    ),
    String,
> {
    let game = crate::games::workspace::snapshot(app, game_id)?;
    let preview = cobble_core::play_manifest::preview(
        &game.directory,
        &game.name,
        &game.version,
        game.loader_kind.as_deref(),
        game.loader_version.as_deref(),
    )
    .await
    .map_err(|_| "MESH_ENVIRONMENT_SHARE_UNAVAILABLE")?;
    let mut document = Document {
        schema_version: 1,
        name: game.name,
        game_version: game.version,
        loader: game.loader_kind,
        loader_version: game.loader_version,
        mods: preview
            .mods
            .iter()
            .map(|item| ModReference {
                source: cobble_content::EnvironmentSource::Modrinth,
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
                name: item.name.clone(),
                sha1: item.sha1.clone(),
                size: item.size,
            })
            .collect(),
        configs: vec![],
        excluded: preview.excluded_mods,
    };
    let root = game.directory;
    let supplements = tauri::async_runtime::spawn_blocking(move || {
        let plan = cobble_core::environment_install::read_environment_plan(&root)
            .map_err(|_| "MESH_ENVIRONMENT_PROVENANCE_INVALID")?;
        let mut items = vec![];
        if let Some(plan) = plan {
            for item in plan.curseforge.into_iter().flat_map(|plan| plan.graph.mods) {
                if let Ok((hash, size)) = local_hash(&root, &item.file.filename)
                    && hash == item.file.sha1
                    && size == item.file.size
                {
                    items.push(ModReference {
                        source: cobble_content::EnvironmentSource::Curseforge,
                        project_id: item.project_id,
                        version_id: item.version_id,
                        name: item.file.filename,
                        sha1: hash,
                        size,
                    });
                }
            }
        }
        Ok::<_, String>(items)
    })
    .await
    .map_err(|_| "MESH_ENVIRONMENT_PROVENANCE_INVALID")??;
    for item in supplements {
        document.excluded.retain(|name| name != &item.name);
        document.mods.push(item);
    }
    document
        .mods
        .sort_by(|a, b| (a.source, &a.project_id).cmp(&(b.source, &b.project_id)));
    validate(&document)?;
    let revision = cobble_plugin_host::digest(
        &serde_json::to_vec(&(&document, &preview.revision))
            .map_err(|_| "MESH_ENVIRONMENT_SHARE_UNAVAILABLE")?,
    );
    Ok((document, preview.configs, revision))
}
pub fn verify_plan(
    document: &Document,
    plan: &cobble_content::EnvironmentPlan,
) -> Result<(), String> {
    validate(document)?;
    if document.game_version != plan.game_version
        || document.loader.as_deref() != Some(plan.loader.as_str())
    {
        return Err("MESH_ENVIRONMENT_PLAN_CHANGED".into());
    }
    let expected = document
        .mods
        .iter()
        .map(|item| {
            (
                (
                    item.source,
                    item.project_id.as_str(),
                    item.version_id.as_str(),
                ),
                (
                    item.name.as_str(),
                    item.sha1.to_ascii_lowercase(),
                    item.size,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeMap::new();
    for (source, graph) in [
        (
            cobble_content::EnvironmentSource::Modrinth,
            plan.modrinth.as_ref(),
        ),
        (
            cobble_content::EnvironmentSource::Curseforge,
            plan.curseforge.as_ref().map(|plan| &plan.graph),
        ),
    ] {
        for item in graph.into_iter().flat_map(|graph| &graph.mods) {
            actual.insert(
                (source, item.project_id.as_str(), item.version_id.as_str()),
                (
                    item.file.filename.as_str(),
                    item.file.sha1.to_ascii_lowercase(),
                    item.file.size,
                ),
            );
        }
    }
    if actual != expected {
        return Err("MESH_ENVIRONMENT_PLAN_CHANGED".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Document {
        Document {
            schema_version: 1,
            name: "World".into(),
            game_version: "1.21.1".into(),
            loader: Some("fabric".into()),
            loader_version: Some("0.16.0".into()),
            mods: vec![],
            configs: vec![],
            excluded: vec![],
        }
    }
    #[test]
    fn document_excludes_urls_paths_personal_options_and_unknown_schema() {
        let mut d = document();
        assert!(validate(&d).is_ok());
        d.configs.push(Config {
            path: "options.txt".into(),
            text: "keys".into(),
            sha256: cobble_plugin_host::digest(b"keys"),
        });
        assert!(validate(&d).is_err());
        d.configs[0].path = "config/../options.txt".into();
        assert!(validate(&d).is_err());
        d.configs[0].path = "config/example.toml".into();
        assert!(validate(&d).is_ok());
        let mut value = serde_json::to_value(d).unwrap();
        value["url"] = json!("https://untrusted.invalid");
        assert!(serde_json::from_value::<Document>(value).is_err());
    }
    #[test]
    fn oversized_or_changed_selected_config_is_rejected() {
        let mut d = document();
        d.configs.push(Config {
            path: "config/test.json".into(),
            text: "changed".into(),
            sha256: cobble_plugin_host::digest(b"original"),
        });
        assert!(validate(&d).is_err());
        d.configs[0].text = "x".repeat(65_537);
        d.configs[0].sha256 = cobble_plugin_host::digest(d.configs[0].text.as_bytes());
        assert!(validate(&d).is_err());
    }
    #[test]
    fn exact_source_version_hash_size_and_complete_dependency_set_are_required() {
        let mut d = document();
        d.mods.push(ModReference {
            source: cobble_content::EnvironmentSource::Modrinth,
            project_id: "project".into(),
            version_id: "version".into(),
            name: "example.jar".into(),
            sha1: "a".repeat(40),
            size: 10,
        });
        let mut plan = cobble_content::EnvironmentPlan {
            game_version: d.game_version.clone(),
            loader: "fabric".into(),
            roots: vec![],
            modrinth: Some(cobble_content::ModrinthPlan {
                project_id: "project".into(),
                version_id: "version".into(),
                mods: vec![cobble_content::ResolvedMod {
                    project_id: "project".into(),
                    version_id: "version".into(),
                    name: "Example".into(),
                    version_number: "1".into(),
                    file: cobble_content::ModrinthFile {
                        url: "https://cdn.modrinth.com/data/project/versions/version/example.jar"
                            .into(),
                        filename: "example.jar".into(),
                        size: 10,
                        sha1: "a".repeat(40),
                        sha512: None,
                    },
                    required_by: None,
                }],
            }),
            curseforge: None,
            dependency_edges: vec![],
        };
        assert!(verify_plan(&d, &plan).is_ok());
        plan.modrinth.as_mut().unwrap().mods[0].file.sha1 = "b".repeat(40);
        assert_eq!(
            verify_plan(&d, &plan).unwrap_err(),
            "MESH_ENVIRONMENT_PLAN_CHANGED"
        );
        plan.modrinth.as_mut().unwrap().mods[0].file.sha1 = "a".repeat(40);
        let extra = plan.modrinth.as_ref().unwrap().mods[0].clone();
        let mut extra = extra;
        extra.project_id = "unshared-dependency".into();
        plan.modrinth.as_mut().unwrap().mods.push(extra);
        assert!(verify_plan(&d, &plan).is_err());
    }
}

// SPDX-License-Identifier: MIT
//! Public Modrinth pack discovery; archives are validated by the MRPACK importer.
use crate::modrinth::{LiveApi, Project, Version, file_with_extension, identifier};
use crate::{BoxError, ModrinthFile, SearchResults};
use serde::Serialize;
use url::Url;

const API: &str = "https://api.modrinth.com/v2/";

#[derive(Clone, Debug, Serialize)]
pub struct PackVersion {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub minecraft_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub version_type: String,
    pub date_published: String,
}

/// Host-only artifact selection; do not accept artifact URLs from the renderer.
#[derive(Clone, Debug)]
pub struct ModrinthPackDownload {
    pub version: PackVersion,
    pub file: ModrinthFile,
}

fn modern(mc: &str) -> bool {
    let components: Vec<_> = mc
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<_, _>>()
        .unwrap_or_default();
    matches!(components.as_slice(), [1,minor,..] if *minor>=17)
        || matches!(components.as_slice(), [major,..] if *major>=26)
}

pub fn supported_pack_target(mc: &str, loader: &str) -> bool {
    match loader {
        "fabric" | "minecraft" => modern(mc),
        "forge" | "neoforge" => mc == "1.21.1",
        _ => false,
    }
}

fn filters(mc: Option<&str>, loader: Option<&str>) -> Result<(), BoxError> {
    if mc.is_some_and(|mc| !modern(mc))
        || loader
            .is_some_and(|loader| !matches!(loader, "fabric" | "forge" | "neoforge" | "minecraft"))
        || mc
            .zip(loader)
            .is_some_and(|(mc, loader)| !supported_pack_target(mc, loader))
    {
        return Err("Unsupported Minecraft version or modpack loader filter".into());
    }
    Ok(())
}

fn facets(mc: Option<&str>, loader: Option<&str>) -> Result<String, BoxError> {
    filters(mc, loader)?;
    let mut facets = vec![
        vec!["project_type:modpack".to_owned()],
        vec![
            "client_side:required".into(),
            "client_side:optional".into(),
            "client_side:unknown".into(),
        ],
    ];
    if let Some(mc) = mc {
        facets.push(vec![format!("versions:{mc}")]);
    }
    facets.push(match loader {
        Some(loader) => vec![format!("categories:{loader}")],
        None => ["fabric", "forge", "neoforge", "minecraft"]
            .map(|loader| format!("categories:{loader}"))
            .into(),
    });
    Ok(serde_json::to_string(&facets)?)
}

pub async fn search_modrinth_packs(
    query: &str,
    mc: Option<&str>,
    loader: Option<&str>,
    offset: u32,
) -> Result<SearchResults, BoxError> {
    if query.len() > 200 || offset > 100_000 {
        return Err("Pack search query or offset is too large".into());
    }
    let mut url = Url::parse(&format!("{API}search"))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("facets", &facets(mc, loader)?)
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", "20")
        .append_pair("index", "relevance");
    LiveApi::new()?.get(url).await
}

async fn project(api: &mut LiveApi, requested: &str) -> Result<Project, BoxError> {
    identifier(requested)?;
    let project: Project = api
        .get(Url::parse(&format!("{API}project/{requested}"))?)
        .await?;
    validate_project(&project)?;
    Ok(project)
}

fn validate_project(project: &Project) -> Result<(), BoxError> {
    identifier(&project.id)?;
    if project.project_type != "modpack"
        || !matches!(
            project.client_side.as_str(),
            "required" | "optional" | "unknown"
        )
    {
        return Err("Project is not a supported client modpack".into());
    }
    Ok(())
}

fn pack_version(
    version: &Version,
    project: &str,
    mc: Option<&str>,
    loader: Option<&str>,
) -> Result<PackVersion, BoxError> {
    if version.project_id != project || version.version_type != "release" {
        return Err("Pack version is not a release belonging to the selected project".into());
    }
    identifier(&version.id)?;
    let minecraft_versions: Vec<_> = version
        .game_versions
        .iter()
        .filter(|game| {
            mc.is_none_or(|mc| mc == game.as_str())
                && version.loaders.iter().any(|kind| {
                    loader.is_none_or(|loader| loader == kind.as_str())
                        && supported_pack_target(game, kind)
                })
        })
        .cloned()
        .collect();
    let loaders: Vec<_> = version
        .loaders
        .iter()
        .filter(|kind| {
            loader.is_none_or(|loader| loader == kind.as_str())
                && minecraft_versions
                    .iter()
                    .any(|mc| supported_pack_target(mc, kind))
        })
        .cloned()
        .collect();
    if minecraft_versions.is_empty() || loaders.is_empty() {
        return Err("Pack version has no supported Minecraft and loader combination".into());
    }
    if file_with_extension(version, ".mrpack")?.size > 512 * 1024 * 1024 {
        return Err("Pack archive exceeds the shared downloader's 512 MiB limit".into());
    }
    Ok(PackVersion {
        version_id: version.id.clone(),
        name: if version.name.is_empty() {
            version.version_number.clone()
        } else {
            version.name.clone()
        },
        version_number: version.version_number.clone(),
        minecraft_versions,
        loaders,
        version_type: version.version_type.clone(),
        date_published: version.date_published.clone(),
    })
}

pub async fn list_modrinth_pack_versions(
    requested: &str,
    mc: Option<&str>,
    loader: Option<&str>,
) -> Result<Vec<PackVersion>, BoxError> {
    filters(mc, loader)?;
    let mut api = LiveApi::new()?;
    let project = project(&mut api, requested).await?;
    let mut url = Url::parse(&format!("{API}project/{}/version", project.id))?;
    if let Some(mc) = mc {
        url.query_pairs_mut()
            .append_pair("game_versions", &serde_json::to_string(&[mc])?);
    }
    if let Some(loader) = loader {
        url.query_pairs_mut()
            .append_pair("loaders", &serde_json::to_string(&[loader])?);
    }
    url.query_pairs_mut()
        .append_pair("include_changelog", "false");
    let versions: Vec<Version> = api.get(url).await?;
    let mut versions: Vec<_> = versions
        .iter()
        .filter_map(|version| pack_version(version, &project.id, mc, loader).ok())
        .collect();
    versions.sort_by(|a, b| {
        b.date_published
            .cmp(&a.date_published)
            .then(b.version_id.cmp(&a.version_id))
    });
    Ok(versions)
}

pub async fn resolve_modrinth_pack(
    requested: &str,
    version_id: &str,
) -> Result<ModrinthPackDownload, BoxError> {
    identifier(version_id)?;
    let mut api = LiveApi::new()?;
    let project = project(&mut api, requested).await?;
    let version: Version = api
        .get(Url::parse(&format!("{API}version/{version_id}"))?)
        .await?;
    if version.id != version_id {
        return Err("Modrinth returned a different pack version".into());
    }
    Ok(ModrinthPackDownload {
        version: pack_version(&version, &project.id, None, None)?,
        file: file_with_extension(&version, ".mrpack")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn version() -> Version {
        serde_json::from_value(json!({"id":"Version","project_id":"Project","name":"Pack release","version_number":"1.0","version_type":"release","date_published":"2026-10-22T00:00:00Z","game_versions":["1.16.5","1.21.1","26.1"],"loaders":["fabric","quilt"],"dependencies":[],"files":[{"url":"https://cdn.modrinth.com/data/Project/versions/Version/pack.mrpack","filename":"pack.mrpack","size":100,"primary":true,"hashes":{"sha1":"0123456789012345678901234567890123456789"}}]})).unwrap()
    }
    #[test]
    fn optional_filters_and_supported_pairs_are_exact() {
        let all: serde_json::Value = serde_json::from_str(&facets(None, None).unwrap()).unwrap();
        assert_eq!(all[0], json!(["project_type:modpack"]));
        assert!(facets(Some("1.21.1"), Some("forge")).is_ok());
        assert!(facets(Some("1.20.1"), Some("forge")).is_err());
        assert!(facets(None, Some("quilt")).is_err());
        let selected = pack_version(&version(), "Project", None, None).unwrap();
        assert_eq!(selected.minecraft_versions, vec!["1.21.1", "26.1"]);
        assert_eq!(selected.loaders, vec!["fabric"]);
        assert!(supported_pack_target("26.1", "minecraft"));
        assert!(!supported_pack_target("1.16.5", "fabric"));
    }
    #[test]
    fn wrong_project_beta_and_unsupported_archives_are_not_installable() {
        let mut version = version();
        assert!(pack_version(&version, "Other", None, None).is_err());
        version.version_type = "beta".into();
        assert!(pack_version(&version, "Project", None, None).is_err());
        version.version_type = "release".into();
        version.loaders = vec!["quilt".into()];
        assert!(pack_version(&version, "Project", None, None).is_err());
        let project:Project=serde_json::from_value(json!({"id":"Project","project_type":"mod","client_side":"optional","title":"Not a pack"})).unwrap();
        assert!(validate_project(&project).is_err());
        let mut raw = json!({"id":"Version","project_id":"Project","version_number":"1.0","version_type":"release","date_published":"now","game_versions":["1.21.1"],"loaders":["fabric"],"dependencies":[],"files":[{"url":"https://cdn.modrinth.com/data/a.zip","filename":"curseforge.zip","size":100,"primary":true,"hashes":{"sha1":"0123456789012345678901234567890123456789"}}]});
        assert!(
            pack_version(
                &serde_json::from_value(raw.clone()).unwrap(),
                "Project",
                None,
                None
            )
            .is_err()
        );
        raw["files"][0]["filename"] = "pack.mrpack".into();
        raw["files"][0]["url"] = "https://example.invalid/pack.mrpack".into();
        assert!(
            pack_version(&serde_json::from_value(raw).unwrap(), "Project", None, None).is_err()
        );
    }
}

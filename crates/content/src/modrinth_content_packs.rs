// SPDX-License-Identifier: MIT
//! Resource/shader ZIP discovery through Modrinth's public v2 API.
use crate::modrinth::{LiveApi, Project, Version, file_with_extension, identifier};
use crate::{BoxError, LocalPackKind, ModrinthFile, SearchResults};
use serde::Serialize;
use url::Url;

const API: &str = "https://api.modrinth.com/v2/";

#[derive(Clone, Debug, Serialize)]
pub struct ContentPackVersion {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
}

/// Host-only artifact resolved from trusted API metadata, never renderer URLs.
#[derive(Clone, Debug)]
pub struct ModrinthContentPackDownload {
    pub version: ContentPackVersion,
    pub file: ModrinthFile,
}

fn project_type(kind: LocalPackKind) -> &'static str {
    match kind {
        LocalPackKind::ResourcePack => "resourcepack",
        LocalPackKind::ShaderPack => "shader",
    }
}

fn target(mc: &str) -> Result<(), BoxError> {
    if mc.is_empty()
        || mc.len() > 64
        || !mc
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("无效的 Minecraft 版本".into());
    }
    Ok(())
}

fn search_url(kind: LocalPackKind, mc: &str, query: &str, offset: u32) -> Result<Url, BoxError> {
    target(mc)?;
    if query.len() > 200 || offset > 100_000 {
        return Err("搜索内容或页数过长".into());
    }
    let facets = serde_json::to_string(&[
        vec![format!("project_type:{}", project_type(kind))],
        vec![format!("versions:{mc}")],
    ])?;
    let mut url = Url::parse(&format!("{API}search"))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("facets", &facets)
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", "20")
        .append_pair("index", "relevance");
    Ok(url)
}

pub async fn search_modrinth_content_packs(
    kind: LocalPackKind,
    mc: &str,
    query: &str,
    offset: u32,
) -> Result<SearchResults, BoxError> {
    LiveApi::new()?
        .get(search_url(kind, mc, query, offset)?)
        .await
}

fn validate_project(project: &Project, kind: LocalPackKind) -> Result<(), BoxError> {
    identifier(&project.id)?;
    if project.project_type != project_type(kind)
        || !matches!(
            project.client_side.as_str(),
            "required" | "optional" | "unknown"
        )
    {
        return Err("所选项目不是这个类型的客户端内容包".into());
    }
    Ok(())
}

async fn project(
    api: &mut LiveApi,
    kind: LocalPackKind,
    requested: &str,
) -> Result<Project, BoxError> {
    identifier(requested)?;
    let project: Project = api
        .get(Url::parse(&format!("{API}project/{requested}"))?)
        .await?;
    validate_project(&project, kind)?;
    Ok(project)
}

fn selected_version(
    version: &Version,
    project: &str,
    mc: &str,
) -> Result<ContentPackVersion, BoxError> {
    identifier(&version.id)?;
    if version.project_id != project
        || !version.game_versions.iter().any(|game| game == mc)
        || !matches!(version.version_type.as_str(), "release" | "beta" | "alpha")
    {
        return Err("所选内容包版本不属于这个项目，或不支持当前 Minecraft 版本".into());
    }
    if file_with_extension(version, ".zip")?.size > 512 * 1024 * 1024 {
        return Err("内容包 ZIP 超过 512 MiB 下载上限".into());
    }
    Ok(ContentPackVersion {
        version_id: version.id.clone(),
        name: if version.name.is_empty() {
            version.version_number.clone()
        } else {
            version.name.clone()
        },
        version_number: version.version_number.clone(),
        version_type: version.version_type.clone(),
        date_published: version.date_published.clone(),
    })
}

pub async fn list_modrinth_content_pack_versions(
    kind: LocalPackKind,
    mc: &str,
    project_id: &str,
) -> Result<Vec<ContentPackVersion>, BoxError> {
    target(mc)?;
    let mut api = LiveApi::new()?;
    let project = project(&mut api, kind, project_id).await?;
    let mut url = Url::parse(&format!("{API}project/{}/version", project.id))?;
    url.query_pairs_mut()
        .append_pair("game_versions", &serde_json::to_string(&[mc])?);
    url.query_pairs_mut()
        .append_pair("include_changelog", "false");
    let versions: Vec<Version> = api.get(url).await?;
    let mut versions: Vec<_> = versions
        .iter()
        .filter_map(|version| selected_version(version, &project.id, mc).ok())
        .collect();
    versions.sort_by(|a, b| {
        b.date_published
            .cmp(&a.date_published)
            .then(b.version_id.cmp(&a.version_id))
    });
    Ok(versions)
}

pub async fn resolve_modrinth_content_pack(
    kind: LocalPackKind,
    mc: &str,
    project_id: &str,
    version_id: &str,
) -> Result<ModrinthContentPackDownload, BoxError> {
    target(mc)?;
    identifier(version_id)?;
    let mut api = LiveApi::new()?;
    let project = project(&mut api, kind, project_id).await?;
    let version: Version = api
        .get(Url::parse(&format!("{API}version/{version_id}"))?)
        .await?;
    if version.id != version_id {
        return Err("Modrinth 返回的内容包版本与选择不一致".into());
    }
    Ok(ModrinthContentPackDownload {
        version: selected_version(&version, &project.id, mc)?,
        file: file_with_extension(&version, ".zip")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn version() -> Value {
        json!({"id":"Version","project_id":"Project","name":"Pack 1.0","version_number":"1.0","version_type":"release","date_published":"2026-10-22T00:00:00Z","game_versions":["1.21.1"],"loaders":["iris","optifine"],"dependencies":[],"files":[{"url":"https://cdn.modrinth.com/data/Project/versions/Version/pack.zip","filename":"pack.zip","size":100,"primary":true,"hashes":{"sha1":"0123456789012345678901234567890123456789"}}]})
    }
    use serde_json::Value;

    #[test]
    fn search_filters_exact_kind_and_game_without_inventing_loader_requirements() {
        for (kind, expected) in [
            (LocalPackKind::ResourcePack, "resourcepack"),
            (LocalPackKind::ShaderPack, "shader"),
        ] {
            let url = search_url(kind, "1.21.1", "test & shader", 20).unwrap();
            let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            let facets: Value = serde_json::from_str(&query["facets"]).unwrap();
            assert_eq!(
                facets,
                json!([[format!("project_type:{expected}")], ["versions:1.21.1"]])
            );
            assert_eq!(query["query"], "test & shader");
            assert_eq!(query["offset"], "20");
            assert!(!query.contains_key("loaders"));
        }
        assert!(
            search_url(
                LocalPackKind::ShaderPack,
                "1.21.1\"],[\"categories:forge",
                "",
                0
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_release_beta_alpha_are_labeled_and_bound_to_game_and_project() {
        for kind in ["release", "beta", "alpha"] {
            let mut value = version();
            value["version_type"] = kind.into();
            let selected =
                selected_version(&serde_json::from_value(value).unwrap(), "Project", "1.21.1")
                    .unwrap();
            assert_eq!(selected.version_type, kind);
            assert_eq!(selected.version_number, "1.0");
        }
        let parsed: Version = serde_json::from_value(version()).unwrap();
        assert!(selected_version(&parsed, "Other", "1.21.1").is_err());
        assert!(selected_version(&parsed, "Project", "1.21").is_err());
        let resource: Project = serde_json::from_value(json!({"id":"Project","project_type":"resourcepack","client_side":"required","title":"Pack"})).unwrap();
        assert!(validate_project(&resource, LocalPackKind::ResourcePack).is_ok());
        assert!(validate_project(&resource, LocalPackKind::ShaderPack).is_err());
    }

    #[test]
    fn wrong_format_unsafe_origin_and_ambiguous_files_are_rejected_before_download() {
        let mut wrong = version();
        wrong["files"][0]["filename"] = "pack.jar".into();
        assert!(
            selected_version(&serde_json::from_value(wrong).unwrap(), "Project", "1.21.1").is_err()
        );
        let mut wrong = version();
        wrong["files"][0]["url"] = "https://untrusted.invalid/pack.zip".into();
        assert!(
            selected_version(&serde_json::from_value(wrong).unwrap(), "Project", "1.21.1").is_err()
        );
        let mut wrong = version();
        let second = wrong["files"][0].clone();
        wrong["files"].as_array_mut().unwrap().push(second);
        assert!(
            selected_version(&serde_json::from_value(wrong).unwrap(), "Project", "1.21.1").is_err()
        );
    }
}

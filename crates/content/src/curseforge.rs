// SPDX-License-Identifier: MIT
//! Official CurseForge v1 metadata and bounded pinned dependency resolution.
//! No persistent API response cache, proxy, mirror or synthesized download URL.
//! Artifact transfer and publication remain in Core.
use cobble_net::{Broker, BrokerConfig, CredentialId, CredentialScope, ProviderCredentials};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fmt;
use url::Url;
#[path = "curseforge_resolve.rs"]
mod resolve;
pub use resolve::{
    InstallPlan, PackArchiveSelection, PackFileDestination, PackInstallPlan, ResolvedPackFile,
    RestrictedFile,
};

const ORIGIN: &str = "https://api.curseforge.com";
const MINECRAFT: u32 = 432;
const PAGE_SIZE: u32 = 50;
const MAX_METADATA: u64 = 8 * 1024 * 1024;

/// API-provided artifact URLs only; this never constructs or rewrites a CDN URL.
pub fn official_file_url(value: &str) -> bool {
    Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https"
            && matches!(
                url.host_str(),
                Some("edge.forgecdn.net" | "mediafilez.forgecdn.net" | "media.forgecdn.net")
            )
            && url.path().starts_with("/files/")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    })
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Mod,
    Modpack,
}
impl ContentKind {
    fn class_id(self) -> u32 {
        match self {
            Self::Mod => 6,
            Self::Modpack => 4471,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ApiError {
    FileValidation {
        project_id: u32,
        file_id: u32,
        field: &'static str,
    },
    UnsupportedClass,
    MissingCredential,
    InvalidInput,
    VersionRequired,
    Network,
    Unauthorized,
    RateLimited,
    NotFound,
    Unavailable,
    InvalidResponse,
}
impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::FileValidation {
            project_id,
            file_id,
            field,
        } = self
        {
            return write!(
                formatter,
                "CurseForge 文件校验失败（项目 {project_id}，文件 {file_id}，{field}）"
            );
        }
        formatter.write_str(match self {
            Self::FileValidation { .. } => "CurseForge 文件校验失败",
            Self::UnsupportedClass => "此 CurseForge 内容类型暂不支持安装",
            Self::MissingCredential => "CurseForge 服务尚未配置",
            Self::InvalidInput => "CurseForge 搜索条件无效",
            Self::VersionRequired => "请先选择 Minecraft 版本，再筛选加载器",
            Self::Network => "无法连接 CurseForge，请稍后重试",
            Self::Unauthorized => "CurseForge 服务授权不可用",
            Self::RateLimited => "CurseForge 请求过于频繁，请稍后重试",
            Self::NotFound => "CurseForge 项目或文件不存在",
            Self::Unavailable => "CurseForge 服务暂时不可用",
            Self::InvalidResponse => "CurseForge 返回了无效数据",
        })
    }
}
impl std::error::Error for ApiError {}

#[derive(Serialize)]
pub struct Project {
    pub icon_url: Option<String>,
    pub preview_url: Option<String>,
    pub gallery: Vec<String>,
    pub class_id: u32,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub website_url: Option<String>,
    pub allow_distribution: Option<bool>,
    pub available: bool,
}
#[derive(Serialize)]
pub struct SearchPage {
    pub hits: Vec<Project>,
    pub offset: u32,
    pub total_hits: u64,
}
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DownloadAvailability {
    Available,
    Manual,
    Unavailable,
}
#[derive(Serialize)]
pub struct Dependency {
    pub project_id: String,
    pub relation: &'static str,
    pub relation_type: u32,
}
#[derive(Serialize)]
pub struct File {
    pub file_id: String,
    pub project_id: String,
    pub name: String,
    pub file_name: String,
    pub release_type: &'static str,
    pub date_published: String,
    pub size_bytes: u64,
    pub game_versions: Vec<String>,
    pub dependencies: Vec<Dependency>,
    pub download_availability: DownloadAvailability,
}
#[derive(Serialize)]
pub struct FilePage {
    pub files: Vec<File>,
    pub offset: u32,
    pub total_hits: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    index: u32,
    page_size: u32,
    result_count: u32,
    total_count: u64,
}
#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    pagination: Pagination,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawProject {
    #[serde(default)]
    logo: Option<RawAsset>,
    #[serde(default)]
    screenshots: Vec<RawAsset>,
    #[serde(default)]
    class_id: u32,
    id: u32,
    game_id: u32,
    name: String,
    slug: String,
    summary: String,
    download_count: serde_json::Number,
    authors: Vec<Author>,
    links: Links,
    allow_mod_distribution: Option<bool>,
    is_available: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAsset {
    #[serde(default)]
    url: String,
    #[serde(default)]
    thumbnail_url: String,
}
impl RawAsset {
    fn thumbnail(&self) -> Option<String> {
        crate::media::optional("curseforge", Some(self.thumbnail_url.clone()))
            .or_else(|| crate::media::optional("curseforge", Some(self.url.clone())))
    }
}
#[derive(Deserialize)]
struct Author {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Links {
    website_url: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    id: u32,
    game_id: u32,
    mod_id: u32,
    is_available: bool,
    display_name: String,
    file_name: String,
    release_type: u32,
    file_date: String,
    file_length: u64,
    download_url: Option<String>,
    game_versions: Vec<String>,
    dependencies: Vec<RawDependency>,
    #[serde(default)]
    hashes: Vec<RawHash>,
}
#[derive(Deserialize)]
struct RawHash {
    value: String,
    algo: u32,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDependency {
    mod_id: u32,
    relation_type: u32,
}

/// Native/Core-owned credentials only. Deliberately has no Debug/Serialize implementation.
pub struct CurseForgeClient {
    broker: Broker,
    credential: CredentialId,
}
impl CurseForgeClient {
    pub fn new(api_key: String) -> Result<Self, ApiError> {
        if api_key.is_empty() {
            return Err(ApiError::MissingCredential);
        }
        if api_key.len() > 4096 || !api_key.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(ApiError::MissingCredential);
        }
        let broker = Broker::new(BrokerConfig {
            max_redirects: 0,
            ..BrokerConfig::default()
        })
        .map_err(|_| ApiError::Network)?;
        let scope = CredentialScope::new(ORIGIN, "/v1").map_err(|_| ApiError::Network)?;
        let credential = broker
            .register_credentials(
                scope,
                ProviderCredentials {
                    bearer: None,
                    api_key: Some(api_key),
                    cookie: None,
                },
            )
            .map_err(|_| ApiError::MissingCredential)?;
        Ok(Self { broker, credential })
    }
    async fn get<T: DeserializeOwned>(&self, url: Url) -> Result<T, ApiError> {
        if url.origin().ascii_serialization() != ORIGIN {
            return Err(ApiError::InvalidInput);
        }
        // `get`, intentionally not the provider metadata-cache or loader-proxy API.
        let response = self
            .broker
            .get(url.as_str(), Some(self.credential.clone()), MAX_METADATA)
            .await
            .map_err(|_| ApiError::Network)?;
        check_status(response.metadata.status)?;
        if response.metadata.final_origin != ORIGIN || response.metadata.redirects != 0 {
            return Err(ApiError::InvalidResponse);
        }
        serde_json::from_slice(&response.body).map_err(|_| ApiError::InvalidResponse)
    }
    pub async fn search(
        &self,
        query: &str,
        kind: ContentKind,
        minecraft_version: Option<&str>,
        loader: Option<&str>,
        offset: u32,
    ) -> Result<SearchPage, ApiError> {
        let url = search_url(query, kind, minecraft_version, loader, offset)?;
        let page: Page<RawProject> = self.get(url).await?;
        validate_page(&page, offset)?;
        let total_hits = page.pagination.total_count.min(10_000);
        let hits = page
            .data
            .into_iter()
            .map(project)
            .collect::<Result<_, _>>()?;
        Ok(SearchPage {
            hits,
            offset,
            total_hits,
        })
    }
    pub async fn files(
        &self,
        project_id: u32,
        minecraft_version: Option<&str>,
        loader: Option<&str>,
        offset: u32,
    ) -> Result<FilePage, ApiError> {
        let url = files_url(project_id, minecraft_version, loader, offset)?;
        let page: Page<RawFile> = self.get(url).await?;
        validate_page(&page, offset)?;
        let total_hits = page.pagination.total_count.min(10_000);
        let files = page
            .data
            .into_iter()
            .map(|value| file(value, project_id))
            .collect::<Result<_, _>>()?;
        Ok(FilePage {
            files,
            offset,
            total_hits,
        })
    }
}
fn check_status(status: u16) -> Result<(), ApiError> {
    match status {
        200 => Ok(()),
        401 | 403 => Err(ApiError::Unauthorized),
        404 => Err(ApiError::NotFound),
        429 => Err(ApiError::RateLimited),
        _ => Err(ApiError::Unavailable),
    }
}
fn validate_page<T>(page: &Page<T>, offset: u32) -> Result<(), ApiError> {
    if page.pagination.index != offset
        || page.pagination.page_size > PAGE_SIZE
        || page.data.len() > PAGE_SIZE as usize
        || page.pagination.result_count as usize != page.data.len()
    {
        return Err(ApiError::InvalidResponse);
    }
    Ok(())
}
fn loader_id(loader: &str) -> Result<&'static str, ApiError> {
    match loader {
        "forge" => Ok("1"),
        "fabric" => Ok("4"),
        "quilt" => Ok("5"),
        "neoforge" => Ok("6"),
        _ => Err(ApiError::InvalidInput),
    }
}
fn filters(
    url: &mut Url,
    version: Option<&str>,
    loader: Option<&str>,
    offset: u32,
) -> Result<(), ApiError> {
    if offset > 10_000 - PAGE_SIZE {
        return Err(ApiError::InvalidInput);
    }
    let mut query = url.query_pairs_mut();
    query
        .append_pair("index", &offset.to_string())
        .append_pair("pageSize", &PAGE_SIZE.to_string());
    if let Some(version) = version {
        if version.is_empty() || version.len() > 80 || version.chars().any(char::is_control) {
            return Err(ApiError::InvalidInput);
        }
        query.append_pair("gameVersion", version);
    }
    if let Some(loader) = loader {
        query.append_pair("modLoaderType", loader_id(loader)?);
    }
    Ok(())
}
fn search_url(
    query: &str,
    kind: ContentKind,
    version: Option<&str>,
    loader: Option<&str>,
    offset: u32,
) -> Result<Url, ApiError> {
    if query.len() > 200 || query.chars().any(char::is_control) {
        return Err(ApiError::InvalidInput);
    }
    if loader.is_some() && version.is_none() {
        return Err(ApiError::VersionRequired);
    }
    let mut url =
        Url::parse(&format!("{ORIGIN}/v1/mods/search")).map_err(|_| ApiError::InvalidInput)?;
    url.query_pairs_mut()
        .append_pair("gameId", &MINECRAFT.to_string())
        .append_pair("classId", &kind.class_id().to_string())
        .append_pair("searchFilter", query)
        .append_pair("sortField", "2")
        .append_pair("sortOrder", "desc");
    filters(&mut url, version, loader, offset)?;
    Ok(url)
}
fn files_url(
    project_id: u32,
    version: Option<&str>,
    loader: Option<&str>,
    offset: u32,
) -> Result<Url, ApiError> {
    if project_id == 0 {
        return Err(ApiError::InvalidInput);
    }
    let mut url = Url::parse(&format!("{ORIGIN}/v1/mods/{project_id}/files"))
        .map_err(|_| ApiError::InvalidInput)?;
    filters(&mut url, version, loader, offset)?;
    Ok(url)
}
fn website(value: Option<String>) -> Option<String> {
    value.filter(|value| {
        Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && matches!(
                    url.host_str(),
                    Some("www.curseforge.com" | "curseforge.com")
                )
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
        })
    })
}
fn project(value: RawProject) -> Result<Project, ApiError> {
    if value.id == 0
        || value.game_id != MINECRAFT
        || value.name.len() > 1024
        || value.summary.len() > 16 * 1024
        || value.authors.len() > 128
    {
        return Err(ApiError::InvalidResponse);
    }
    let downloads = value
        .download_count
        .as_u64()
        .or_else(|| {
            value
                .download_count
                .as_f64()
                .filter(|count| {
                    count.is_finite()
                        && *count >= 0.0
                        && count.fract() == 0.0
                        && *count < u64::MAX as f64
                })
                .map(|count| count as u64)
        })
        .ok_or(ApiError::InvalidResponse)?;
    let icon_url = value.logo.as_ref().and_then(RawAsset::thumbnail);
    let preview_url = value
        .screenshots
        .iter()
        .take(64)
        .find_map(RawAsset::thumbnail);
    let gallery = crate::media::gallery(
        "curseforge",
        value
            .screenshots
            .into_iter()
            .map(|asset| asset.url)
            .collect(),
    );
    Ok(Project {
        icon_url,
        preview_url,
        gallery,
        class_id: value.class_id,
        project_id: value.id.to_string(),
        slug: value.slug,
        title: value.name,
        description: value.summary,
        author: value
            .authors
            .into_iter()
            .map(|author| author.name)
            .collect::<Vec<_>>()
            .join(", "),
        downloads,
        website_url: website(value.links.website_url),
        allow_distribution: value.allow_mod_distribution,
        available: value.is_available,
    })
}
fn file(value: RawFile, project_id: u32) -> Result<File, ApiError> {
    if value.id == 0
        || value.mod_id != project_id
        || value.game_id != MINECRAFT
        || value.file_name.len() > 255
        || value.file_name.is_empty()
        || value
            .file_name
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
        || value.dependencies.len() > 1024
        || value.game_versions.len() > 512
    {
        return Err(ApiError::InvalidResponse);
    }
    let download_availability = if !value.is_available {
        DownloadAvailability::Unavailable
    } else if value.download_url.as_deref().is_some_and(|value| {
        Url::parse(value).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
    }) {
        DownloadAvailability::Available
    } else {
        DownloadAvailability::Manual
    };
    Ok(File {
        file_id: value.id.to_string(),
        project_id: value.mod_id.to_string(),
        name: value.display_name,
        file_name: value.file_name,
        release_type: match value.release_type {
            1 => "release",
            2 => "beta",
            3 => "alpha",
            _ => "unknown",
        },
        date_published: value.file_date,
        size_bytes: value.file_length,
        game_versions: value.game_versions,
        dependencies: value
            .dependencies
            .into_iter()
            .map(|dependency| Dependency {
                project_id: dependency.mod_id.to_string(),
                relation: match dependency.relation_type {
                    1 => "embedded",
                    2 => "optional",
                    3 => "required",
                    4 => "tool",
                    5 => "incompatible",
                    6 => "include",
                    _ => "unknown",
                },
                relation_type: dependency.relation_type,
            })
            .collect(),
        download_availability,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "Explicit official API credential required; metadata only, no cache"]
    fn official_minecraft_class_ids_match_install_routes() {
        use super::*;
        #[derive(Deserialize)]
        struct Classes {
            data: Vec<Class>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Class {
            id: u32,
            game_id: u32,
            is_class: bool,
            slug: String,
        }
        let key =
            std::env::var("COURSE_FORGE_API").expect("explicit development credential required");
        let client = CurseForgeClient::new(key).unwrap();
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let classes: Classes = client
                    .get(
                        Url::parse(
                            "https://api.curseforge.com/v1/categories?gameId=432&classesOnly=true",
                        )
                        .unwrap(),
                    )
                    .await
                    .unwrap();
                for (id, slug) in [
                    (6, "mc-mods"),
                    (12, "texture-packs"),
                    (4471, "modpacks"),
                    (6552, "shaders"),
                ] {
                    assert!(
                        classes.data.iter().any(|class| class.id == id
                            && class.game_id == 432
                            && class.is_class
                            && class.slug == slug),
                        "official class route differs"
                    );
                }
            });
    }
    use super::*;
    #[test]
    fn search_is_fixed_origin_encoded_and_respects_official_filter_limits() {
        let url = search_url(
            "a & b",
            ContentKind::Modpack,
            Some("1.21.1"),
            Some("neoforge"),
            50,
        )
        .unwrap();
        assert_eq!(url.host_str(), Some("api.curseforge.com"));
        let pairs = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(pairs.get("classId").unwrap(), "4471");
        assert_eq!(pairs.get("searchFilter").unwrap(), "a & b");
        assert_eq!(pairs.get("modLoaderType").unwrap(), "6");
        assert!(matches!(
            search_url("", ContentKind::Mod, None, Some("fabric"), 0),
            Err(ApiError::VersionRequired)
        ));
        assert!(files_url(1, None, None, 9951).is_err());
        assert!(files_url(0, None, None, 0).is_err());
    }
    #[test]
    fn blocked_downloads_stay_manual_and_dependencies_remain_typed() {
        let value = serde_json::json!({"id":10,"gameId":432,"modId":1,"isAvailable":true,"displayName":"A","fileName":"a.jar","releaseType":1,"fileDate":"2026-01-01T00:00:00Z","fileLength":42,"downloadUrl":null,"gameVersions":["1.21.1","Fabric"],"dependencies":[{"modId":2,"relationType":3}]});
        let parsed = file(serde_json::from_value(value.clone()).unwrap(), 1).unwrap();
        assert_eq!(parsed.download_availability, DownloadAvailability::Manual);
        assert_eq!(parsed.dependencies[0].relation, "required");
        assert!(
            !serde_json::to_string(&parsed)
                .unwrap()
                .contains("download_url")
        );
        assert!(file(serde_json::from_value(value).unwrap(), 2).is_err());
        assert!(website(Some("https://unrelated.invalid/project".into())).is_none());
    }
    #[test]
    fn credential_and_http_errors_are_static_and_safe() {
        let secret = "synthetic-sensitive-key";
        assert!(CurseForgeClient::new(format!("{secret}\r\nextra")).is_err());
        for status in [401, 403, 404, 429, 500] {
            assert!(
                !check_status(status)
                    .unwrap_err()
                    .to_string()
                    .contains(secret)
            );
        }
    }
}

#[cfg(test)]
mod artwork_tests {
    use super::*;
    #[test]
    fn search_project_artwork_uses_official_logo_and_screenshots() {
        let raw:RawProject=serde_json::from_value(serde_json::json!({"id":1,"gameId":432,"classId":4471,"name":"Original fixture","slug":"fixture","summary":"Pack","downloadCount":1,"authors":[],"links":{},"allowModDistribution":true,"isAvailable":true,"logo":{"url":"https://media.forgecdn.net/avatars/1/logo.png","thumbnailUrl":"https://media.forgecdn.net/avatars/thumbnails/1/64/logo.png"},"screenshots":[{"url":"https://media.forgecdn.net/attachments/1/screen.png","thumbnailUrl":"https://media.forgecdn.net/attachments/thumbnails/1/screen.png"},{"url":"https://evil.example/tracker.png"}]})).unwrap();
        let normalized = project(raw).unwrap();
        assert!(normalized.icon_url.unwrap().contains("/thumbnails/"));
        assert!(
            normalized
                .preview_url
                .unwrap()
                .contains("/attachments/thumbnails/")
        );
        assert_eq!(
            normalized.gallery,
            ["https://media.forgecdn.net/attachments/1/screen.png"]
        );
    }
}

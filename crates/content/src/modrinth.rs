// SPDX-License-Identifier: MIT
//! Independently implemented against Modrinth's public v2 API documentation.
//! A bounded release selection slice, not a general mod compatibility solver.

use crate::BoxError;
use cobble_net::{Broker, BrokerConfig};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use url::Url;

const API: &str = "https://api.modrinth.com/v2/";
const MAX_PROJECTS: usize = 128;
const MAX_REQUESTS: usize = 512;
const MAX_METADATA: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResults {
    pub hits: Vec<ModProject>,
    pub offset: u32,
    pub total_hits: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModProject {
    #[serde(default, deserialize_with = "crate::media::modrinth_image")]
    pub icon_url: Option<String>,
    #[serde(
        default,
        alias = "featured_gallery",
        deserialize_with = "crate::media::modrinth_image"
    )]
    pub preview_url: Option<String>,
    #[serde(default, deserialize_with = "crate::media::modrinth_gallery")]
    pub gallery: Vec<String>,
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModrinthPlan {
    pub project_id: String,
    pub version_id: String,
    pub mods: Vec<ResolvedMod>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedMod {
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub file: ModrinthFile,
    pub required_by: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModrinthFile {
    pub url: String,
    pub filename: String,
    pub size: u64,
    pub sha1: String,
    pub sha512: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ModrinthUpdate {
    pub current_version: String,
    pub selected: ResolvedMod,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModVersion {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModVersionDetails {
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub changelog: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModVersionSelection {
    pub project_id: String,
    pub version_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedMod {
    pub project_id: String,
    pub version_id: String,
    pub sha1: String,
}

fn error(message: impl Into<String>) -> BoxError {
    io::Error::other(message.into()).into()
}
pub(super) fn identifier(value: &str) -> Result<(), BoxError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(error("Invalid Modrinth ID"));
    }
    Ok(())
}
fn target(mc: &str, loader: &str) -> Result<(), BoxError> {
    if !matches!(loader, "fabric" | "forge" | "neoforge")
        || mc.is_empty()
        || mc.len() > 64
        || !mc
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(error(
            "Modrinth browsing supports Fabric, Forge and NeoForge; the Minecraft version must be a valid version identifier",
        ));
    }
    Ok(())
}

/// Public mod search; no credentials and no download URLs cross this boundary.
pub async fn search_modrinth(
    query: &str,
    mc: &str,
    loader: &str,
    offset: u32,
) -> Result<SearchResults, BoxError> {
    target(mc, loader)?;
    if query.len() > 200 || offset > 100_000 {
        return Err(error("Search query or offset is too large"));
    }
    let facets = search_facets(mc, loader)?;
    let mut url = Url::parse(&format!("{API}search"))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("facets", &facets)
        .append_pair("offset", &offset.to_string())
        .append_pair("limit", "20")
        .append_pair("index", "relevance");
    let mut api = LiveApi::new()?;
    api.get(url).await
}

fn search_facets(mc: &str, loader: &str) -> Result<String, BoxError> {
    target(mc, loader)?;
    Ok(serde_json::to_string(&vec![
        vec!["project_type:mod".to_string()],
        vec![format!("versions:{mc}")],
        vec![format!("categories:{loader}")],
        vec![
            "client_side:required".to_string(),
            "client_side:optional".to_string(),
            "client_side:unknown".to_string(),
        ],
    ])?)
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    pub version_type: String,
    pub date_published: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    changelog: Option<String>,
    dependencies: Vec<Dependency>,
    files: Vec<ApiFile>,
}
#[derive(Clone, Debug, Deserialize)]
struct Dependency {
    project_id: Option<String>,
    version_id: Option<String>,
    dependency_type: String,
}
#[derive(Clone, Debug, Deserialize)]
struct ApiFile {
    url: String,
    filename: String,
    size: u64,
    hashes: HashMap<String, String>,
    primary: bool,
}
#[derive(Deserialize)]
pub(super) struct Project {
    pub id: String,
    pub project_type: String,
    pub client_side: String,
    title: String,
}
trait Api {
    async fn versions(
        &mut self,
        project: &str,
        mc: &str,
        loader: &str,
    ) -> Result<Vec<Version>, BoxError>;
    async fn version(&mut self, version: &str) -> Result<Version, BoxError>;
    async fn project(&mut self, project: &str) -> Result<Project, BoxError>;
    async fn versions_from_hash(&mut self, _hash: &str) -> Result<Vec<Version>, BoxError> {
        Err(error(
            "Hash identification is unavailable for this API adapter",
        ))
    }
}
pub(super) struct LiveApi {
    broker: Broker,
    requests: usize,
}
impl LiveApi {
    pub fn new() -> Result<Self, BoxError> {
        Ok(Self {
            broker: Broker::new(BrokerConfig::default())?,
            requests: 0,
        })
    }
    pub async fn get<T: DeserializeOwned>(&mut self, url: Url) -> Result<T, BoxError> {
        self.get_optional(url)
            .await?
            .ok_or_else(|| error("Modrinth: project or version not found (HTTP 404)"))
    }

    async fn get_optional<T: DeserializeOwned>(&mut self, url: Url) -> Result<Option<T>, BoxError> {
        self.requests += 1;
        if self.requests > MAX_REQUESTS {
            return Err(error(
                "Modrinth resolution request limit reached; reduce the selected dependency graph",
            ));
        }
        let response = self
            .broker
            .get_public_metadata(url.as_str(), "modrinth-v2", MAX_METADATA)
            .await?;
        if response.metadata.status == 404 {
            return Ok(None);
        }
        if response.metadata.status != 200 {
            let reason = match response.metadata.status {
                404 => "project or version not found",
                429 => "rate limit reached; retry later",
                401 | 403 => "access denied",
                _ => "upstream request failed",
            };
            return Err(error(format!(
                "Modrinth: {reason} (HTTP {})",
                response.metadata.status
            )));
        }
        match serde_json::from_slice(&response.body) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                if let Some(cache) = cobble_net::PublicMetadataCache::shared() {
                    let _ = cache
                        .invalidate(url.as_str(), "modrinth-v2", MAX_METADATA)
                        .await;
                }
                Err(error.into())
            }
        }
    }
}
impl Api for LiveApi {
    async fn versions(
        &mut self,
        project: &str,
        mc: &str,
        loader: &str,
    ) -> Result<Vec<Version>, BoxError> {
        identifier(project)?;
        let mut url = Url::parse(&format!("{API}project/{project}/version"))?;
        url.query_pairs_mut()
            .append_pair("game_versions", &serde_json::to_string(&[mc])?)
            .append_pair("loaders", &serde_json::to_string(&[loader])?)
            .append_pair("include_changelog", "false");
        self.get(url).await
    }
    async fn version(&mut self, version: &str) -> Result<Version, BoxError> {
        identifier(version)?;
        self.get(Url::parse(&format!("{API}version/{version}"))?)
            .await
    }
    async fn project(&mut self, project: &str) -> Result<Project, BoxError> {
        identifier(project)?;
        self.get(Url::parse(&format!("{API}project/{project}"))?)
            .await
    }
    async fn versions_from_hash(&mut self, hash: &str) -> Result<Vec<Version>, BoxError> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Matches {
            One(Box<Version>),
            Many(Vec<Version>),
        }
        let mut url = Url::parse(&format!("{API}version_file/{hash}"))?;
        url.query_pairs_mut()
            .append_pair("algorithm", "sha1")
            .append_pair("multiple", "true");
        Ok(match self.get_optional::<Matches>(url).await? {
            None => Vec::new(),
            Some(Matches::One(version)) => vec![*version],
            Some(Matches::Many(versions)) => versions,
        })
    }
}

/// Match bytes to compatible client-Mod metadata without trusting a local name.
/// The public API currently returns a single object even with multiple=true;
/// arrays are accepted defensively and ambiguous returned identities skipped.
pub async fn identify_modrinth_file(
    hash: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<IdentifiedMod>, BoxError> {
    let broker = Broker::new(BrokerConfig::default())?;
    identify_modrinth_file_with_broker(&broker, hash, mc, loader).await
}

/// Reuse a broker for bounded concurrent identification of a local collection.
pub async fn identify_modrinth_file_with_broker(
    broker: &Broker,
    hash: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<IdentifiedMod>, BoxError> {
    target(mc, loader)?;
    if !valid_hash(hash, 40) {
        return Err(error("Invalid local Mod SHA-1"));
    }
    identify_file(
        &mut LiveApi {
            broker: broker.clone(),
            requests: 0,
        },
        hash,
        mc,
        loader,
    )
    .await
}

async fn identify_file(
    api: &mut impl Api,
    hash: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<IdentifiedMod>, BoxError> {
    let versions = api.versions_from_hash(hash).await?;
    let mut matches = BTreeMap::new();
    for version in versions {
        if !compatible(&version, mc, loader) {
            continue;
        }
        identifier(&version.id)?;
        identifier(&version.project_id)?;
        // Recognition follows the same playable primary-file selection as
        // updates, avoiding adoption of sources/docs/auxiliary JARs.
        let Ok(selected) = file(&version) else {
            continue;
        };
        if !selected.sha1.eq_ignore_ascii_case(hash) {
            continue;
        }
        matches.insert((version.project_id.clone(), version.id.clone()), version);
    }
    if matches.len() != 1 {
        return Ok(None);
    }
    let (_, version) = matches
        .into_iter()
        .next()
        .ok_or_else(|| error("Missing hash match"))?;
    let project = api.project(&version.project_id).await?;
    if project.id != version.project_id
        || project.project_type != "mod"
        || !matches!(
            project.client_side.as_str(),
            "required" | "optional" | "unknown"
        )
    {
        return Ok(None);
    }
    Ok(Some(IdentifiedMod {
        project_id: version.project_id,
        version_id: version.id,
        sha1: hash.to_ascii_lowercase(),
    }))
}

/// Select latest compatible releases, then required dependencies. Explicit pins
/// override unconstrained choices; conflicting pins or incompatible edges fail.
/// No alternate-release search or claims of runtime mod compatibility are made.
pub async fn resolve_modrinth(
    project_id: &str,
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    target(mc, loader)?;
    identifier(project_id)?;
    let mut api = LiveApi::new()?;
    let project = api.project(project_id).await?;
    identifier(&project.id)?;
    resolve(&mut api, &project.id, mc, loader).await
}

/// Joint resolution for explicitly selected update roots. Their latest releases
/// stay pinned so a dependency cannot silently downgrade another selected root.
pub async fn resolve_modrinth_many(
    project_ids: &[String],
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    target(mc, loader)?;
    let mut api = LiveApi::new()?;
    resolve_many(&mut api, project_ids, mc, loader).await
}

/// Resolve user-selected roots exactly, including explicitly selected prereleases.
/// Unpinned dependencies continue to select compatible stable releases.
pub async fn resolve_modrinth_selected(
    selections: &[ModVersionSelection],
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    target(mc, loader)?;
    let mut api = LiveApi::new()?;
    resolve_selected(&mut api, selections, mc, loader).await
}

pub async fn list_modrinth_versions(
    project_id: &str,
    mc: &str,
    loader: &str,
) -> Result<Vec<ModVersion>, BoxError> {
    target(mc, loader)?;
    identifier(project_id)?;
    let mut api = LiveApi::new()?;
    list_versions(&mut api, project_id, mc, loader).await
}

pub async fn modrinth_version_details(
    project_id: &str,
    version_id: &str,
    mc: &str,
    loader: &str,
) -> Result<ModVersionDetails, BoxError> {
    target(mc, loader)?;
    identifier(project_id)?;
    identifier(version_id)?;
    let mut api = LiveApi::new()?;
    version_details(&mut api, project_id, version_id, mc, loader).await
}

fn supported_client_project(project: &Project) -> Result<(), BoxError> {
    identifier(&project.id)?;
    if project.project_type != "mod"
        || !matches!(
            project.client_side.as_str(),
            "required" | "optional" | "unknown"
        )
    {
        return Err(error("Project is not a supported client mod"));
    }
    Ok(())
}

fn version_summary(
    version: &Version,
    project: &str,
    mc: &str,
    loader: &str,
) -> Result<ModVersion, BoxError> {
    identifier(&version.id)?;
    if version.project_id != project
        || !compatible(version, mc, loader)
        || !matches!(version.version_type.as_str(), "release" | "beta" | "alpha")
    {
        return Err(error(
            "Selected Mod version is incompatible or belongs to another project",
        ));
    }
    file(version)?;
    Ok(ModVersion {
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

async fn list_versions(
    api: &mut impl Api,
    requested: &str,
    mc: &str,
    loader: &str,
) -> Result<Vec<ModVersion>, BoxError> {
    let project = api.project(requested).await?;
    supported_client_project(&project)?;
    let versions = api.versions(&project.id, mc, loader).await?;
    let mut versions: Vec<_> = versions
        .iter()
        .filter_map(|version| version_summary(version, &project.id, mc, loader).ok())
        .collect();
    versions.sort_by(|a, b| {
        b.date_published
            .cmp(&a.date_published)
            .then(b.version_id.cmp(&a.version_id))
    });
    Ok(versions)
}

async fn version_details(
    api: &mut impl Api,
    requested: &str,
    version_id: &str,
    mc: &str,
    loader: &str,
) -> Result<ModVersionDetails, BoxError> {
    let project = api.project(requested).await?;
    supported_client_project(&project)?;
    let version = api.version(version_id).await?;
    if version.id != version_id {
        return Err(error("Modrinth returned a different selected version"));
    }
    let summary = version_summary(&version, &project.id, mc, loader)?;
    Ok(ModVersionDetails {
        version_id: summary.version_id,
        name: summary.name,
        version_number: summary.version_number,
        version_type: summary.version_type,
        date_published: summary.date_published,
        changelog: version.changelog.unwrap_or_default(),
    })
}

pub async fn find_modrinth_update(
    project_id: &str,
    current_version: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<ModrinthUpdate>, BoxError> {
    target(mc, loader)?;
    identifier(project_id)?;
    identifier(current_version)?;
    let mut api = LiveApi::new()?;
    find_update(&mut api, project_id, current_version, mc, loader).await
}

/// Read the display version of an installed managed Mod, validating its project.
pub async fn modrinth_version_number(
    project_id: &str,
    version_id: &str,
) -> Result<String, BoxError> {
    identifier(project_id)?;
    identifier(version_id)?;
    let version = LiveApi::new()?.version(version_id).await?;
    if version.id != version_id || version.project_id != project_id {
        return Err(error(
            "Installed Modrinth version does not belong to this project",
        ));
    }
    Ok(version.version_number)
}

fn latest_release<'a>(
    versions: &'a [Version],
    project: &str,
    mc: &str,
    loader: &str,
) -> Option<&'a Version> {
    versions
        .iter()
        .filter(|version| {
            version.project_id == project
                && version.version_type == "release"
                && compatible(version, mc, loader)
        })
        .max_by(|a, b| {
            a.date_published
                .cmp(&b.date_published)
                .then(a.id.cmp(&b.id))
        })
}

async fn find_update(
    api: &mut impl Api,
    project_id: &str,
    current_version: &str,
    mc: &str,
    loader: &str,
) -> Result<Option<ModrinthUpdate>, BoxError> {
    let current = api.version(current_version).await?;
    if current.id != current_version || current.project_id != project_id {
        return Err(error(
            "Installed Modrinth version does not belong to this project",
        ));
    }
    let versions = api.versions(project_id, mc, loader).await?;
    let Some(next) = latest_release(&versions, project_id, mc, loader) else {
        return Ok(None);
    };
    if next.id == current.id || next.date_published <= current.date_published {
        return Ok(None);
    }
    let project = api.project(project_id).await?;
    if project.id != project_id
        || project.project_type != "mod"
        || !matches!(
            project.client_side.as_str(),
            "required" | "optional" | "unknown"
        )
    {
        return Err(error("Project is not a supported client mod"));
    }
    Ok(Some(ModrinthUpdate {
        current_version: current.version_number,
        selected: ResolvedMod {
            project_id: project_id.to_owned(),
            version_id: next.id.clone(),
            name: project.title,
            version_number: next.version_number.clone(),
            file: file(next)?,
            required_by: None,
        },
    }))
}
fn compatible(version: &Version, mc: &str, loader: &str) -> bool {
    version.game_versions.iter().any(|v| v == mc) && version.loaders.iter().any(|v| v == loader)
}
fn file(version: &Version) -> Result<ModrinthFile, BoxError> {
    file_with_extension(version, ".jar")
}

pub(super) fn file_with_extension(
    version: &Version,
    extension: &str,
) -> Result<ModrinthFile, BoxError> {
    let primary: Vec<_> = version.files.iter().filter(|f| f.primary).collect();
    let selected = match primary.as_slice() {
        [one] => *one,
        [] if version.files.len() == 1 => &version.files[0],
        _ => {
            return Err(error(format!(
                "Version {} has ambiguous primary files",
                version.id
            )));
        }
    };
    let url = Url::parse(&selected.url)?;
    if url.scheme() != "https"
        || url.host_str() != Some("cdn.modrinth.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(error("Unsupported Modrinth artifact origin"));
    }
    let filename = &selected.filename;
    if filename.len() > 240
        || !filename.to_ascii_lowercase().ends_with(extension)
        || filename.contains(['/', '\\', ':'])
        || filename.chars().any(char::is_control)
    {
        return Err(error("Invalid Modrinth artifact filename"));
    }
    let sha1 = selected
        .hashes
        .get("sha1")
        .filter(|v| valid_hash(v, 40))
        .ok_or_else(|| error("Missing or invalid Modrinth SHA-1"))?
        .clone();
    let sha512 = selected.hashes.get("sha512").cloned();
    if sha512.as_ref().is_some_and(|v| !valid_hash(v, 128))
        || selected.size == 0
        || selected.size > 1024 * 1024 * 1024
    {
        return Err(error("Invalid Modrinth artifact hash or size"));
    }
    Ok(ModrinthFile {
        url: selected.url.clone(),
        filename: filename.clone(),
        size: selected.size,
        sha1,
        sha512,
    })
}
fn valid_hash(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}

async fn resolve(
    api: &mut impl Api,
    root: &str,
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    resolve_many(api, &[root.to_owned()], mc, loader).await
}

async fn resolve_many(
    api: &mut impl Api,
    roots: &[String],
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    resolve_roots(api, roots, None, mc, loader).await
}

async fn resolve_selected(
    api: &mut impl Api,
    selections: &[ModVersionSelection],
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    if selections.is_empty() || selections.len() > MAX_PROJECTS {
        return Err(error("Select between 1 and 128 Modrinth projects"));
    }
    let mut pins = BTreeMap::new();
    let mut roots = Vec::new();
    for selection in selections {
        identifier(&selection.project_id)?;
        identifier(&selection.version_id)?;
        if pins
            .insert(selection.project_id.clone(), selection.version_id.clone())
            .is_some()
        {
            return Err(error("Duplicate selected Modrinth project"));
        }
        roots.push(selection.project_id.clone());
    }
    resolve_roots(api, &roots, Some(&pins), mc, loader).await
}

async fn resolve_roots(
    api: &mut impl Api,
    roots: &[String],
    selected_pins: Option<&BTreeMap<String, String>>,
    mc: &str,
    loader: &str,
) -> Result<ModrinthPlan, BoxError> {
    if roots.is_empty() || roots.len() > MAX_PROJECTS {
        return Err(error("Select between 1 and 128 Modrinth projects"));
    }
    let root = &roots[0];
    let mut versions = HashMap::<String, Vec<Version>>::new();
    let mut exact = HashMap::<String, Version>::new();
    let mut projects = HashMap::<String, Project>::new();
    let mut root_pins = BTreeMap::<String, String>::new();
    for root in roots {
        identifier(root)?;
        if root_pins.contains_key(root) {
            return Err(error("Duplicate selected Modrinth project"));
        }
        let chosen = if let Some(pins) = selected_pins {
            let pin = pins
                .get(root)
                .ok_or_else(|| error("Missing selected root pin"))?;
            let version = api.version(pin).await?;
            if version.id != *pin {
                return Err(error("Modrinth returned a different selected version"));
            }
            version_summary(&version, root, mc, loader)?;
            version
        } else {
            let choices = api.versions(root, mc, loader).await?;
            let chosen = latest_release(&choices, root, mc, loader)
                .ok_or_else(|| {
                    error(format!(
                        "No compatible release for project {root} on Minecraft {mc} / {loader}"
                    ))
                })?
                .clone();
            versions.insert(root.clone(), choices);
            chosen
        };
        root_pins.insert(root.clone(), chosen.id.clone());
        exact.insert(chosen.id.clone(), chosen);
    }
    let mut pins = root_pins.clone();
    for _ in 0..MAX_PROJECTS {
        let mut selected = BTreeMap::<String, (Version, Option<String>)>::new();
        let mut discovered = root_pins.clone();
        let mut queue: VecDeque<_> = roots.iter().map(|root| (root.clone(), None)).collect();
        while let Some((id, parent)) = queue.pop_front() {
            if selected.contains_key(&id) {
                continue;
            }
            identifier(&id)?;
            if selected.len() >= MAX_PROJECTS {
                return Err(error("Modrinth dependency graph exceeds 128 projects"));
            }
            if !projects.contains_key(&id) {
                projects.insert(id.clone(), api.project(&id).await?);
            }
            let project = &projects[&id];
            if project.id != id
                || project.project_type != "mod"
                || !matches!(
                    project.client_side.as_str(),
                    "required" | "optional" | "unknown"
                )
            {
                return Err(error(format!("Project {id} is not a supported client mod")));
            }
            let chosen = if let Some(pin) = pins.get(&id) {
                if !exact.contains_key(pin) {
                    exact.insert(pin.clone(), api.version(pin).await?);
                }
                let version = exact[pin].clone();
                if version.id != *pin
                    || version.project_id != id
                    || !compatible(&version, mc, loader)
                {
                    return Err(error(format!(
                        "Pinned version {pin} is incompatible with {id}, Minecraft {mc}, or {loader}"
                    )));
                }
                version
            } else {
                if !versions.contains_key(&id) {
                    versions.insert(id.clone(), api.versions(&id, mc, loader).await?);
                }
                latest_release(&versions[&id], &id, mc, loader)
                    .cloned()
                    .ok_or_else(|| {
                        error(format!(
                            "No compatible release for project {id} on Minecraft {mc} / {loader}"
                        ))
                    })?
            };
            identifier(&chosen.id)?;
            if chosen.dependencies.len() > MAX_PROJECTS {
                return Err(error("Too many Modrinth dependencies"));
            }
            for dependency in &chosen.dependencies {
                if !matches!(
                    dependency.dependency_type.as_str(),
                    "required" | "optional" | "embedded" | "incompatible"
                ) {
                    return Err(error("Unknown Modrinth dependency type"));
                }
                if dependency.dependency_type != "required" {
                    continue;
                }
                let dependency_id = if let Some(pin) = &dependency.version_id {
                    identifier(pin)?;
                    if !exact.contains_key(pin) {
                        exact.insert(pin.clone(), api.version(pin).await?);
                    }
                    let version = &exact[pin];
                    if version.id != *pin
                        || !compatible(version, mc, loader)
                        || dependency
                            .project_id
                            .as_ref()
                            .is_some_and(|p| p != &version.project_id)
                    {
                        return Err(error(format!(
                            "Required pin {pin} of {id} has incompatible target metadata"
                        )));
                    }
                    let dep_id = version.project_id.clone();
                    if let Some(previous) = discovered.insert(dep_id.clone(), pin.clone())
                        && previous != *pin
                    {
                        return Err(error(format!(
                            "Conflicting required pins for {dep_id}: {previous} and {pin} (required by {id})"
                        )));
                    }
                    dep_id
                } else {
                    dependency.project_id.clone().ok_or_else(|| {
                        error(format!(
                            "Required dependency of {id} has no project or version ID"
                        ))
                    })?
                };
                queue.push_back((dependency_id, Some(id.clone())));
            }
            selected.insert(id, (chosen, parent));
        }
        if discovered != pins {
            pins = discovered;
            continue;
        }
        for (id, (version, _)) in &selected {
            for dep in version
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "incompatible")
            {
                if selected.values().any(|(v, _)| {
                    dep.version_id.as_ref().map_or_else(
                        || dep.project_id.as_ref() == Some(&v.project_id),
                        |pin| pin == &v.id,
                    )
                }) {
                    return Err(error(format!(
                        "Project {id} declares an incompatibility with a selected dependency"
                    )));
                }
            }
        }
        let version_id = selected[root].0.id.clone();
        let mods = selected
            .into_iter()
            .map(|(id, (version, parent))| {
                Ok(ResolvedMod {
                    file: file(&version)?,
                    project_id: id.clone(),
                    version_id: version.id,
                    name: projects[&id].title.clone(),
                    version_number: version.version_number,
                    required_by: parent,
                })
            })
            .collect::<Result<_, BoxError>>()?;
        return Ok(ModrinthPlan {
            project_id: root.to_string(),
            version_id,
            mods,
        });
    }
    Err(error(
        "Modrinth exact dependency pins did not stabilize; reduce the selected graph",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        versions: Vec<Version>,
    }
    impl Api for Fixture {
        async fn versions(
            &mut self,
            project: &str,
            _: &str,
            _: &str,
        ) -> Result<Vec<Version>, BoxError> {
            Ok(self
                .versions
                .iter()
                .filter(|v| v.project_id == project)
                .cloned()
                .collect())
        }
        async fn version(&mut self, version: &str) -> Result<Version, BoxError> {
            self.versions
                .iter()
                .find(|v| v.id == version)
                .cloned()
                .ok_or_else(|| error("Fixture version missing"))
        }
        async fn project(&mut self, project: &str) -> Result<Project, BoxError> {
            Ok(Project {
                id: project.to_string(),
                title: format!("Project {project}"),
                client_side: if project == "Server" {
                    "unsupported"
                } else {
                    "optional"
                }
                .into(),
                project_type: "mod".into(),
            })
        }
        async fn versions_from_hash(&mut self, _: &str) -> Result<Vec<Version>, BoxError> {
            Ok(self.versions.clone())
        }
    }
    fn version(project: &str, id: &str, date: &str, deps: serde_json::Value) -> Version {
        serde_json::from_value(json!({"project_id":project,"id":id,"version_number":id,"version_type":"release","date_published":date,"game_versions":["1.21.1"],"loaders":["fabric"],"changelog":null,"dependencies":deps,"files":[{"url":"https://cdn.modrinth.com/data/Example/versions/Example/example.jar","filename":format!("{project}.jar"),"size":100,"primary":true,"hashes":{"sha1":"0123456789012345678901234567890123456789"}}]})).unwrap()
    }

    fn selection(project: &str, version: &str) -> ModVersionSelection {
        ModVersionSelection {
            project_id: project.into(),
            version_id: version.into(),
        }
    }

    #[test]
    fn explicit_alpha_root_stays_pinned_and_unpinned_dependency_stays_stable() {
        let mut alpha = version(
            "Root",
            "Alpha",
            "2026-01-01",
            json!([{"project_id":"Lib","dependency_type":"required"}]),
        );
        alpha.version_type = "alpha".into();
        let stable = version("Root", "Latest", "2026-04-01", json!([]));
        let lib = version("Lib", "LibStable", "2026-02-01", json!([]));
        let mut beta = version("Lib", "LibBeta", "2026-05-01", json!([]));
        beta.version_type = "beta".into();
        let mut api = Fixture {
            versions: vec![alpha, stable, lib, beta],
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let plan = runtime
            .block_on(resolve_selected(
                &mut api,
                &[selection("Root", "Alpha")],
                "1.21.1",
                "fabric",
            ))
            .unwrap();
        assert_eq!(plan.version_id, "Alpha");
        assert_eq!(
            plan.mods
                .iter()
                .find(|item| item.project_id == "Lib")
                .unwrap()
                .version_id,
            "LibStable"
        );
        let latest = runtime
            .block_on(resolve_many(&mut api, &["Root".into()], "1.21.1", "fabric"))
            .unwrap();
        assert_eq!(latest.version_id, "Latest");
        api.versions[0].dependencies = serde_json::from_value(
            json!([{"project_id":"Lib","version_id":"LibStable","dependency_type":"required"}]),
        )
        .unwrap();
        let error = runtime
            .block_on(resolve_selected(
                &mut api,
                &[selection("Root", "Alpha"), selection("Lib", "LibBeta")],
                "1.21.1",
                "fabric",
            ))
            .unwrap_err();
        assert!(error.to_string().contains("Conflicting required pins"));
    }

    #[test]
    fn listed_versions_and_changelog_validate_project_game_loader_and_nullable_metadata() {
        let mut root = version("Root", "Beta", "2026-02-01", json!([]));
        root.version_type = "beta".into();
        root.changelog = Some("## Changes\nFixed an issue".into());
        let mut incompatible = version("Root", "WrongLoader", "2026-03-01", json!([]));
        incompatible.loaders = vec!["forge".into()];
        let older = version("Root", "Release", "2026-01-01", json!([]));
        let mut api = Fixture {
            versions: vec![root, incompatible, older],
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let listed = runtime
            .block_on(list_versions(&mut api, "Root", "1.21.1", "fabric"))
            .unwrap();
        assert_eq!(
            listed
                .iter()
                .map(|item| item.version_id.as_str())
                .collect::<Vec<_>>(),
            vec!["Beta", "Release"]
        );
        let details = runtime
            .block_on(version_details(
                &mut api, "Root", "Beta", "1.21.1", "fabric",
            ))
            .unwrap();
        assert_eq!(details.changelog, "## Changes\nFixed an issue");
        assert_eq!(
            serde_json::to_value(details).unwrap()["version_type"],
            "beta"
        );
        let details = runtime
            .block_on(version_details(
                &mut api, "Root", "Release", "1.21.1", "fabric",
            ))
            .unwrap();
        assert!(details.changelog.is_empty());
        for (project, mc, loader) in [
            ("Other", "1.21.1", "fabric"),
            ("Root", "1.21", "fabric"),
            ("Root", "1.21.1", "forge"),
            ("Server", "1.21.1", "fabric"),
        ] {
            assert!(
                runtime
                    .block_on(version_details(&mut api, project, "Beta", mc, loader))
                    .is_err()
            );
        }
        let resource: Project = serde_json::from_value(json!({"id":"Root","project_type":"resourcepack","client_side":"required","title":"Pack"})).unwrap();
        assert!(supported_client_project(&resource).is_err());
    }

    #[test]
    fn selected_roots_never_substitute_a_foreign_or_incompatible_version() {
        let mut api = Fixture {
            versions: vec![version("Root", "RootVersion", "2026-01-01", json!([]))],
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(
            runtime
                .block_on(resolve_selected(
                    &mut api,
                    &[selection("Other", "RootVersion")],
                    "1.21.1",
                    "fabric"
                ))
                .is_err()
        );
        assert!(
            runtime
                .block_on(resolve_selected(
                    &mut api,
                    &[selection("Root", "RootVersion")],
                    "1.20.1",
                    "fabric"
                ))
                .is_err()
        );
        assert!(
            runtime
                .block_on(resolve_selected(
                    &mut api,
                    &[selection("Root", "RootVersion")],
                    "1.21.1",
                    "forge"
                ))
                .is_err()
        );
        assert!(
            runtime
                .block_on(resolve_selected(
                    &mut api,
                    &[
                        selection("Root", "RootVersion"),
                        selection("Root", "RootVersion")
                    ],
                    "1.21.1",
                    "fabric"
                ))
                .is_err()
        );
    }

    #[test]
    fn hash_recognition_requires_exact_compatible_client_mod_and_skips_ambiguity() {
        let hash = "0123456789012345678901234567890123456789";
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let root = version("Root", "RootV", "2026-01-01", json!([]));
        let mut api = Fixture {
            versions: vec![root.clone()],
        };
        let identified = runtime
            .block_on(identify_file(&mut api, hash, "1.21.1", "fabric"))
            .unwrap()
            .unwrap();
        assert_eq!(identified.project_id, "Root");
        assert_eq!(identified.version_id, "RootV");
        assert_eq!(identified.sha1, hash);
        assert!(
            runtime
                .block_on(identify_file(&mut api, &"0".repeat(40), "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
        assert!(
            runtime
                .block_on(identify_file(&mut api, hash, "1.21.1", "forge"))
                .unwrap()
                .is_none()
        );
        api.versions
            .push(version("Root", "OtherVersion", "2026-01-02", json!([])));
        assert!(
            runtime
                .block_on(identify_file(&mut api, hash, "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
        api.versions = vec![
            root,
            version("OtherProject", "OtherVersion", "2026-01-02", json!([])),
        ];
        assert!(
            runtime
                .block_on(identify_file(&mut api, hash, "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
        api.versions = vec![version("Server", "ServerV", "2026-01-01", json!([]))];
        assert!(
            runtime
                .block_on(identify_file(&mut api, hash, "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
        api.versions.clear();
        assert!(
            runtime
                .block_on(identify_file(&mut api, hash, "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn update_check_never_downgrades_a_newer_beta_to_an_older_release() {
        let mut beta = version("Root", "Beta", "2026-03-01", json!([]));
        beta.version_type = "beta".into();
        let old_release = version("Root", "Old", "2026-02-01", json!([]));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut api = Fixture {
            versions: vec![beta.clone(), old_release],
        };
        assert!(
            runtime
                .block_on(find_update(&mut api, "Root", "Beta", "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
        api.versions
            .push(version("Root", "New", "2026-04-01", json!([])));
        let update = runtime
            .block_on(find_update(&mut api, "Root", "Beta", "1.21.1", "fabric"))
            .unwrap()
            .unwrap();
        assert_eq!(update.current_version, "Beta");
        assert_eq!(update.selected.version_id, "New");
        assert!(
            runtime
                .block_on(find_update(&mut api, "Root", "New", "1.21.1", "fabric"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn jointly_resolved_roots_deduplicate_dependencies_and_reject_cross_root_pins() {
        let root = version(
            "Root",
            "RootV",
            "2026-03-01",
            json!([{"project_id":"Lib","dependency_type":"required"}]),
        );
        let other = version(
            "Other",
            "OtherV",
            "2026-03-01",
            json!([{"project_id":"Lib","dependency_type":"required"}]),
        );
        let lib = version("Lib", "LibV", "2026-03-01", json!([]));
        let roots = vec!["Root".to_owned(), "Other".to_owned()];
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let mut api = Fixture {
            versions: vec![root.clone(), other, lib],
        };
        let plan = runtime
            .block_on(resolve_many(&mut api, &roots, "1.21.1", "fabric"))
            .unwrap();
        assert_eq!(plan.mods.len(), 3);
        assert_eq!(
            plan.mods
                .iter()
                .filter(|item| item.project_id == "Lib")
                .count(),
            1
        );
        let old_other = version("Other", "OldOther", "2026-01-01", json!([]));
        api.versions.push(old_other);
        api.versions[0].dependencies = serde_json::from_value(
            json!([{"project_id":"Other","version_id":"OldOther","dependency_type":"required"}]),
        )
        .unwrap();
        let error = runtime
            .block_on(resolve_many(&mut api, &roots, "1.21.1", "fabric"))
            .unwrap_err();
        assert!(error.to_string().contains("Conflicting required pins"));
    }
    fn run(versions: Vec<Version>) -> Result<ModrinthPlan, BoxError> {
        run_for_loader(versions, "fabric")
    }
    fn run_for_loader(versions: Vec<Version>, loader: &str) -> Result<ModrinthPlan, BoxError> {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(resolve(&mut Fixture { versions }, "Root", "1.21.1", loader))
    }
    #[test]
    fn required_cycle_is_deduplicated_optional_and_embedded_skipped() {
        let root = version(
            "Root",
            "RootV",
            "2026-01-01",
            json!([{"project_id":"Lib","dependency_type":"required"},{"project_id":"Optional","dependency_type":"optional"},{"project_id":"Embedded","dependency_type":"embedded"}]),
        );
        let lib = version(
            "Lib",
            "LibV",
            "2026-01-01",
            json!([{"project_id":"Root","dependency_type":"required"}]),
        );
        let plan = run(vec![root, lib]).unwrap();
        assert_eq!(plan.mods.len(), 2);
        assert_eq!(
            plan.mods
                .iter()
                .find(|m| m.project_id == "Lib")
                .unwrap()
                .required_by
                .as_deref(),
            Some("Root")
        );
    }
    #[test]
    fn late_exact_pin_replaces_newest_and_prunes_old_dependency_branch() {
        let root = version(
            "Root",
            "RootV",
            "2026-01-01",
            json!([{"project_id":"Lib","dependency_type":"required"},{"project_id":"Other","dependency_type":"required"}]),
        );
        let old = version("Lib", "Old", "2025-01-01", json!([]));
        let new = version(
            "Lib",
            "New",
            "2026-01-01",
            json!([{"project_id":"Extra","dependency_type":"required"}]),
        );
        let extra = version("Extra", "ExtraV", "2026-01-01", json!([]));
        let other = version(
            "Other",
            "OtherV",
            "2026-01-01",
            json!([{"project_id":"Lib","version_id":"Old","dependency_type":"required"}]),
        );
        let plan = run(vec![root, old, new, extra, other]).unwrap();
        assert_eq!(plan.mods.len(), 3);
        assert_eq!(
            plan.mods
                .iter()
                .find(|m| m.project_id == "Lib")
                .unwrap()
                .version_id,
            "Old"
        );
    }
    #[test]
    fn conflicts_and_wrong_target_pins_return_explanations() {
        let root = version(
            "Root",
            "RootV",
            "2026-01-01",
            json!([{"project_id":"Lib","version_id":"Old","dependency_type":"required"},{"project_id":"Lib","version_id":"New","dependency_type":"required"}]),
        );
        let old = version("Lib", "Old", "2025-01-01", json!([]));
        let mut new = version("Lib", "New", "2026-01-01", json!([]));
        assert!(
            run(vec![root.clone(), old.clone(), new.clone()])
                .unwrap_err()
                .to_string()
                .contains("Conflicting required pins")
        );
        new.game_versions = vec!["1.20.1".into()];
        assert!(
            run(vec![root, old, new])
                .unwrap_err()
                .to_string()
                .contains("incompatible target metadata")
        );
    }
    #[test]
    fn rejects_incompatible_graph_and_does_not_fall_back_to_beta() {
        let root = version(
            "Root",
            "RootV",
            "2026-01-01",
            json!([{"project_id":"Lib","dependency_type":"required"},{"project_id":"Lib","dependency_type":"incompatible"}]),
        );
        let lib = version("Lib", "LibV", "2026-01-01", json!([]));
        assert!(
            run(vec![root, lib])
                .unwrap_err()
                .to_string()
                .contains("incompatibility")
        );
        let mut beta = version("Root", "Beta", "2026-01-01", json!([]));
        beta.version_type = "beta".into();
        assert!(
            run(vec![beta])
                .unwrap_err()
                .to_string()
                .contains("No compatible release")
        );
    }
    #[test]
    fn rejects_untrusted_files_and_unsupported_targets() {
        let mut root = version("Root", "RootV", "2026-01-01", json!([]));
        root.files[0].url = "https://example.invalid/mod.jar".into();
        assert!(file(&root).is_err());
        root.files[0].url = "https://cdn.modrinth.com/data/mod.jar".into();
        root.files[0].filename = "../escape.jar".into();
        assert!(file(&root).is_err());
        assert!(target("1.21.1", "quilt").is_err());
        assert!(target("26.1", "fabric").is_ok());
        assert!(target("../version", "fabric").is_err());
        assert!(identifier("../../version").is_err());
    }

    #[test]
    fn supported_loader_search_facets_and_resolution_are_exact() {
        for loader in ["fabric", "forge", "neoforge"] {
            let facets: serde_json::Value =
                serde_json::from_str(&search_facets("1.21.1", loader).unwrap()).unwrap();
            assert_eq!(facets[1], json!(["versions:1.21.1"]));
            assert_eq!(facets[2], json!([format!("categories:{loader}")]));
            let mut selected = version("Root", "Correct", "2026-01-01", json!([]));
            selected.loaders = vec![loader.into()];
            let mut wrong = selected.clone();
            wrong.id = "WrongLoader".into();
            wrong.date_published = "2026-12-01".into();
            wrong.loaders = vec![
                if loader == "fabric" {
                    "forge"
                } else {
                    "fabric"
                }
                .into(),
            ];
            let plan = run_for_loader(vec![wrong.clone(), selected], loader).unwrap();
            assert_eq!(plan.version_id, "Correct");
            assert!(run_for_loader(vec![wrong], loader).is_err());
        }
    }

    #[test]
    fn forge_and_neoforge_required_pins_retain_loader_filtering() {
        for loader in ["forge", "neoforge"] {
            let mut root = version(
                "Root",
                "RootV",
                "2026-01-01",
                json!([{"project_id":"Lib","version_id":"LibV","dependency_type":"required"}]),
            );
            root.loaders = vec![loader.into()];
            let mut lib = version("Lib", "LibV", "2026-01-01", json!([]));
            assert!(
                run_for_loader(vec![root.clone(), lib.clone()], loader)
                    .unwrap_err()
                    .to_string()
                    .contains("incompatible target metadata")
            );
            lib.loaders = vec![loader.into()];
            assert_eq!(
                run_for_loader(vec![root, lib], loader).unwrap().mods.len(),
                2
            );
        }
    }
}

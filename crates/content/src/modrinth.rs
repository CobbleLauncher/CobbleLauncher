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

fn error(message: impl Into<String>) -> BoxError {
    io::Error::other(message.into()).into()
}
fn identifier(value: &str) -> Result<(), BoxError> {
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
struct Version {
    id: String,
    project_id: String,
    version_number: String,
    version_type: String,
    date_published: String,
    game_versions: Vec<String>,
    loaders: Vec<String>,
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
struct Project {
    id: String,
    project_type: String,
    client_side: String,
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
}
struct LiveApi {
    broker: Broker,
    requests: usize,
}
impl LiveApi {
    fn new() -> Result<Self, BoxError> {
        Ok(Self {
            broker: Broker::new(BrokerConfig::default())?,
            requests: 0,
        })
    }
    async fn get<T: DeserializeOwned>(&mut self, url: Url) -> Result<T, BoxError> {
        self.requests += 1;
        if self.requests > MAX_REQUESTS {
            return Err(error(
                "Modrinth resolution request limit reached; reduce the selected dependency graph",
            ));
        }
        let response = self.broker.get(url.as_str(), None, MAX_METADATA).await?;
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
        Ok(serde_json::from_slice(&response.body)?)
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
            .append_pair("loaders", &serde_json::to_string(&[loader])?);
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
fn compatible(version: &Version, mc: &str, loader: &str) -> bool {
    version.game_versions.iter().any(|v| v == mc) && version.loaders.iter().any(|v| v == loader)
}
fn file(version: &Version) -> Result<ModrinthFile, BoxError> {
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
        || !filename.to_ascii_lowercase().ends_with(".jar")
        || filename.contains(['/', '\\', ':'])
        || filename.chars().any(char::is_control)
    {
        return Err(error("Invalid Modrinth JAR filename"));
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
    let mut versions = HashMap::<String, Vec<Version>>::new();
    let mut exact = HashMap::<String, Version>::new();
    let mut projects = HashMap::<String, Project>::new();
    let mut pins = BTreeMap::<String, String>::new();
    for _ in 0..MAX_PROJECTS {
        let mut selected = BTreeMap::<String, (Version, Option<String>)>::new();
        let mut discovered = BTreeMap::<String, String>::new();
        let mut queue = VecDeque::from([(root.to_string(), None)]);
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
                versions[&id]
                    .iter()
                    .filter(|v| {
                        v.project_id == id
                            && v.version_type == "release"
                            && compatible(v, mc, loader)
                    })
                    .max_by(|a, b| {
                        a.date_published
                            .cmp(&b.date_published)
                            .then(a.id.cmp(&b.id))
                    })
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
                client_side: "optional".into(),
                project_type: "mod".into(),
            })
        }
    }
    fn version(project: &str, id: &str, date: &str, deps: serde_json::Value) -> Version {
        serde_json::from_value(json!({"project_id":project,"id":id,"version_number":id,"version_type":"release","date_published":date,"game_versions":["1.21.1"],"loaders":["fabric"],"dependencies":deps,"files":[{"url":"https://cdn.modrinth.com/data/Example/versions/Example/example.jar","filename":format!("{project}.jar"),"size":100,"primary":true,"hashes":{"sha1":"0123456789012345678901234567890123456789"}}]})).unwrap()
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

// SPDX-License-Identifier: MIT
//! Official file/project identity adapter for the shared finite solver.
//! Authenticated responses live only in this request-local cache.
use super::*;
use crate::{
    BoxError, ModrinthFile, ResolvedMod,
    dependency_solver::{Candidate, VersionConstraint},
    environment::{
        EnvironmentRoot, EnvironmentSource,
        catalog::{Artifact, Catalog, key},
    },
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_PROJECTS: usize = 256;
const MAX_REQUESTS: usize = 512;
const MAX_CANDIDATES: usize = 4096;
const MAX_PER_PROJECT: usize = 32;

pub(crate) trait Api {
    async fn project(&mut self, project: u32) -> Result<Project, ApiError>;
    async fn exact(&mut self, project: u32, file: u32) -> Result<RawFile, ApiError>;
    async fn page(
        &mut self,
        project: u32,
        mc: Option<&str>,
        loader: Option<&str>,
        offset: u32,
    ) -> Result<(Vec<RawFile>, u64), ApiError>;
}
impl Api for &CurseForgeClient {
    async fn project(&mut self, project: u32) -> Result<Project, ApiError> {
        CurseForgeClient::project(self, project).await
    }
    async fn exact(&mut self, project: u32, file: u32) -> Result<RawFile, ApiError> {
        self.exact_file(project, file).await
    }
    async fn page(
        &mut self,
        project: u32,
        mc: Option<&str>,
        loader: Option<&str>,
        offset: u32,
    ) -> Result<(Vec<RawFile>, u64), ApiError> {
        let page: Page<RawFile> = self.get(files_url(project, mc, loader, offset)?).await?;
        validate_page(&page, offset)?;
        Ok((page.data, page.pagination.total_count))
    }
}
type PageKey = (u32, Option<String>, Option<String>, u32);
type FilePage = (Vec<RawFile>, u64);
pub(crate) struct Cache<A> {
    api: A,
    projects: BTreeMap<u32, Project>,
    exact: BTreeMap<(u32, u32), RawFile>,
    pages: BTreeMap<PageKey, FilePage>,
    pub requests: usize,
}
impl<A: Api> Cache<A> {
    pub fn new(api: A) -> Self {
        Self {
            api,
            projects: BTreeMap::new(),
            exact: BTreeMap::new(),
            pages: BTreeMap::new(),
            requests: 0,
        }
    }
    fn request(&mut self) -> Result<(), ApiError> {
        if self.requests >= MAX_REQUESTS {
            return Err(ApiError::Resolution {
                code: "candidate_limit".into(),
                message: "CurseForge metadata request limit reached".into(),
                projects: vec![],
            });
        }
        self.requests += 1;
        Ok(())
    }
    async fn project(&mut self, id: u32) -> Result<Project, ApiError> {
        if let Some(v) = self.projects.get(&id) {
            return Ok(v.clone());
        }
        self.request()?;
        let value = self.api.project(id).await?;
        if value.project_id != id.to_string() {
            return Err(ApiError::InvalidResponse);
        }
        self.projects.insert(id, value.clone());
        Ok(value)
    }
    async fn exact(&mut self, project: u32, id: u32) -> Result<RawFile, ApiError> {
        if let Some(v) = self.exact.get(&(project, id)) {
            return Ok(v.clone());
        }
        self.request()?;
        let value = self.api.exact(project, id).await?;
        if value.id != id || value.mod_id != project || value.game_id != MINECRAFT {
            return Err(ApiError::InvalidResponse);
        }
        self.exact.insert((project, id), value.clone());
        Ok(value)
    }
    async fn page(
        &mut self,
        project: u32,
        mc: Option<&str>,
        loader: Option<&str>,
        offset: u32,
    ) -> Result<(Vec<RawFile>, u64), ApiError> {
        let key = (
            project,
            mc.map(str::to_owned),
            loader.map(str::to_owned),
            offset,
        );
        if let Some(v) = self.pages.get(&key) {
            return Ok(v.clone());
        }
        self.request()?;
        let values = self.api.page(project, mc, loader, offset).await?;
        for v in &values.0 {
            if v.mod_id != project || v.game_id != MINECRAFT || v.id == 0 {
                return Err(ApiError::InvalidResponse);
            }
            self.exact.insert((project, v.id), v.clone());
        }
        self.pages.insert(key, values.clone());
        Ok(values)
    }
    pub(crate) async fn root_targets(
        &mut self,
        root: &EnvironmentRoot,
    ) -> Result<(BTreeSet<String>, bool), BoxError> {
        let project = parse_id(&root.project_id)?;
        let metadata = self.project(project).await?;
        if metadata.class_id != 6 {
            return Err(ApiError::UnsupportedClass.into());
        }
        let (files, truncated) = if let Some(pin) = &root.version_id {
            (vec![self.exact(project, parse_id(pin)?).await?], false)
        } else {
            let mut files = Vec::new();
            let mut truncated = false;
            for page in 0..4 {
                let (values, total) = self.page(project, None, None, page * PAGE_SIZE).await?;
                files.extend(values);
                if files.len() as u64 >= total {
                    break;
                }
                if page == 3 {
                    truncated = true;
                }
            }
            (files, truncated)
        };
        let versions = files
            .iter()
            .filter(|f| root.version_id.is_some() || (f.release_type == 1 && f.is_available))
            .flat_map(|f| f.game_versions.iter())
            .filter(|v| {
                v.split('.')
                    .all(|s| !s.is_empty() && s.parse::<u32>().is_ok())
            })
            .cloned()
            .collect();
        Ok((versions, truncated))
    }
    pub(crate) async fn validate_mod_roots(
        &mut self,
        roots: &[EnvironmentRoot],
    ) -> Result<(), ApiError> {
        for root in roots {
            if self.project(parse_id(&root.project_id)?).await?.class_id != 6 {
                return Err(ApiError::UnsupportedClass);
            }
        }
        Ok(())
    }
}
pub(crate) fn parse_id(value: &str) -> Result<u32, ApiError> {
    let id = value.parse::<u32>().map_err(|_| ApiError::InvalidInput)?;
    if id == 0 || id.to_string() != value {
        return Err(ApiError::InvalidInput);
    }
    Ok(id)
}
fn destination(project: &Project) -> Result<PackFileDestination, ApiError> {
    match project.class_id {
        6 => Ok(PackFileDestination::Mods),
        12 => Ok(PackFileDestination::ResourcePack),
        6552 => Ok(PackFileDestination::ShaderPack),
        _ => Err(ApiError::UnsupportedClass),
    }
}
fn compatible(file: &RawFile, mc: &str, loader: Option<&str>) -> bool {
    let label = match loader {
        Some("fabric") => Some("Fabric"),
        Some("forge") => Some("Forge"),
        Some("neoforge") => Some("NeoForge"),
        Some("quilt") => Some("Quilt"),
        None => None,
        _ => return false,
    };
    file.game_versions.iter().any(|v| v == mc)
        && label.is_none_or(|l| file.game_versions.iter().any(|v| v.eq_ignore_ascii_case(l)))
}
fn artifact(
    project: &Project,
    file: &RawFile,
    destination: PackFileDestination,
) -> Result<Artifact, ApiError> {
    let invalid = |field| ApiError::FileValidation {
        project_id: file.mod_id,
        file_id: file.id,
        field,
    };
    let sha1 = file
        .hashes
        .iter()
        .find(|h| {
            h.algo == 1 && h.value.len() == 40 && h.value.bytes().all(|b| b.is_ascii_hexdigit())
        })
        .ok_or_else(|| invalid("sha1"))?
        .value
        .to_lowercase();
    let (url, restricted) = super::resolve::artifact_route(project, file)?;
    let file_data = ModrinthFile {
        url,
        filename: file.file_name.clone(),
        size: file.file_length,
        sha1,
        sha512: None,
    };
    if !crate::environment::catalog::checked_file(&file_data)
        || file_data.size > 512 * 1024 * 1024
        || !file_data.filename.to_ascii_lowercase().ends_with(
            if destination == PackFileDestination::Mods {
                ".jar"
            } else {
                ".zip"
            },
        )
    {
        return Err(invalid("file_name_or_size"));
    }
    Ok(Artifact {
        resolved: ResolvedMod {
            project_id: file.mod_id.to_string(),
            version_id: file.id.to_string(),
            name: project.title.clone(),
            version_number: file.display_name.clone(),
            file: file_data,
            required_by: None,
        },
        destination,
        restricted,
        known_mod_ids: BTreeSet::new(),
    })
}

pub(crate) async fn collect<A: Api>(
    cache: &mut Cache<A>,
    roots: &[EnvironmentRoot],
    mc: &str,
    loader: Option<&str>,
) -> Result<Catalog, BoxError> {
    if let Some(l) = loader {
        loader_id(l)?;
    }
    let before = cache.requests;
    let mut catalog = Catalog::default();
    let mut pending: VecDeque<u32> = roots
        .iter()
        .map(|r| parse_id(&r.project_id))
        .collect::<Result<_, _>>()?;
    let pins: BTreeMap<_, _> = roots
        .iter()
        .filter_map(|r| {
            r.version_id
                .as_ref()
                .map(|v| Ok((parse_id(&r.project_id)?, parse_id(v)?)))
        })
        .collect::<Result<_, ApiError>>()?;
    let mut visited = BTreeSet::new();
    while let Some(project_id) = pending.pop_front() {
        if !visited.insert(project_id) {
            continue;
        }
        if visited.len() > MAX_PROJECTS {
            return Err(std::io::Error::other(
                "candidate_limit: CurseForge graph exceeds256projects",
            )
            .into());
        }
        let project = cache.project(project_id).await?;
        let destination = destination(&project)?;
        let file_loader = if destination == PackFileDestination::Mods {
            Some(loader.ok_or(ApiError::InvalidInput)?)
        } else {
            None
        };
        let mut files = if let Some(pin) = pins.get(&project_id) {
            vec![cache.exact(project_id, *pin).await?]
        } else {
            let (files, total) = cache.page(project_id, Some(mc), file_loader, 0).await?;
            let mut files: Vec<_> = files
                .into_iter()
                .filter(|f| f.is_available && f.release_type == 1 && compatible(f, mc, file_loader))
                .collect();
            files.sort_by(|a, b| b.file_date.cmp(&a.file_date).then(b.id.cmp(&a.id)));
            catalog.truncated |= files.len() > MAX_PER_PROJECT || total > PAGE_SIZE as u64;
            files.truncate(MAX_PER_PROJECT);
            files
        };
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date).then(b.id.cmp(&a.id)));
        for (preference, file) in files.into_iter().enumerate() {
            if catalog.request.candidates.len() >= MAX_CANDIDATES {
                return Err(std::io::Error::other(
                    "candidate_limit: CurseForge graph exceeds4096candidates",
                )
                .into());
            }
            let package = key(EnvironmentSource::Curseforge, &project_id.to_string());
            let mut candidate = Candidate {
                package: package.clone(),
                version: file.id.to_string(),
                preference,
                required: vec![],
                incompatible: vec![],
                unavailable: None,
            };
            if !compatible(&file, mc, file_loader) {
                candidate.unavailable =
                    Some("Pinned CurseForge file has incompatible game or loader metadata".into());
            }
            if file.dependencies.len() > 256 {
                candidate.unavailable = Some("Too many CurseForge dependencies".into());
            } else {
                for dep in &file.dependencies {
                    if dep.mod_id == 0 {
                        candidate.unavailable =
                            Some("Invalid CurseForge dependency project".into());
                        continue;
                    }
                    let constraint = VersionConstraint::any(key(
                        EnvironmentSource::Curseforge,
                        &dep.mod_id.to_string(),
                    ));
                    match dep.relation_type {
                        3 => {
                            pending.push_back(dep.mod_id);
                            candidate.required.push(constraint);
                        }
                        5 => candidate.incompatible.push(constraint),
                        1 | 2 | 4 | 6 => (),
                        _ => {
                            candidate.unavailable = Some("Unknown CurseForge relation type".into())
                        }
                    }
                }
            }
            match artifact(&project, &file, destination) {
                Ok(artifact) => {
                    catalog
                        .artifacts
                        .insert((package, candidate.version.clone()), artifact);
                }
                Err(e) => candidate.unavailable = Some(e.to_string()),
            }
            catalog.request.candidates.push(candidate);
        }
    }
    catalog.request.roots = roots
        .iter()
        .map(|r| VersionConstraint::any(key(r.source, &r.project_id)))
        .collect();
    catalog.request.pins = roots
        .iter()
        .filter_map(|r| {
            r.version_id
                .as_ref()
                .map(|v| VersionConstraint::exact(key(r.source, &r.project_id), v.clone()))
        })
        .collect();
    catalog.metadata_requests = cache.requests - before;
    Ok(catalog)
}

pub(super) async fn resolve_pinned(
    client: &CurseForgeClient,
    pins: &[(u32, u32)],
    mc: &str,
    loader: Option<&str>,
) -> Result<PackInstallPlan, ApiError> {
    if pins.is_empty() || pins.len() > 256 {
        return Err(ApiError::InvalidInput);
    }
    let roots: Vec<_> = pins
        .iter()
        .map(|(p, v)| EnvironmentRoot {
            source: EnvironmentSource::Curseforge,
            project_id: p.to_string(),
            version_id: Some(v.to_string()),
        })
        .collect();
    if roots
        .iter()
        .map(|r| &r.project_id)
        .collect::<BTreeSet<_>>()
        .len()
        != roots.len()
    {
        return Err(ApiError::InvalidInput);
    }
    let result = async {
        let catalog = collect(&mut Cache::new(client), &roots, mc, loader).await?;
        // Pack destinations share the same dependency solver but retain their
        // destination-specific plans instead of converting everything to mods.
        let artifacts = catalog.artifacts.clone();
        let (plan, _, _) =
            crate::environment::catalog::solve_catalog(catalog, &roots, mc, loader.unwrap_or(""))
                .await?;
        let cf = plan
            .curseforge
            .ok_or_else(|| std::io::Error::other("Missing CurseForge environment plan"))?;
        let files = cf
            .graph
            .mods
            .into_iter()
            .map(|m| {
                let a = &artifacts[&(
                    key(EnvironmentSource::Curseforge, &m.project_id),
                    m.version_id.clone(),
                )];
                ResolvedPackFile {
                    project_id: m.project_id,
                    file_id: m.version_id,
                    file: m.file,
                    destination: a.destination,
                    name: m.name,
                    version_number: m.version_number,
                    required_by: m.required_by,
                }
            })
            .collect();
        Ok::<_, BoxError>(PackInstallPlan {
            files,
            restricted: cf.restricted,
            incompatible: cf.incompatible,
        })
    }
    .await;
    result.map_err(|e| {
        if let Some(api) = e.downcast_ref::<ApiError>() {
            return api.clone();
        }
        let code = e
            .downcast_ref::<crate::dependency_solver::SolverConflict>()
            .map(|c| c.code.clone())
            .unwrap_or_else(|| {
                if e.to_string().starts_with("candidate_limit:") {
                    "candidate_limit".into()
                } else {
                    "metadata_unavailable".into()
                }
            });
        ApiError::Resolution {
            code,
            message: e.to_string(),
            projects: roots.iter().map(|r| r.project_id.clone()).collect(),
        }
    })
}

#[cfg(test)]
mod tests;

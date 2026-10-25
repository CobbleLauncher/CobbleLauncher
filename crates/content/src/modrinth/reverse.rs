// SPDX-License-Identifier: MIT
use super::*;
use std::{collections::BTreeSet, time::Instant};

const MAX_TARGETS: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseConfiguration {
    pub choices: Vec<ReverseConfigurationChoice>,
    pub conflicts: Vec<ReverseConfigurationConflict>,
    pub stats: ReverseConfigurationStats,
    /// Stable codes describe bounded discovery and metadata limits.
    pub limitations: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseConfigurationChoice {
    pub game_version: String,
    pub loader: String,
    pub roots: Vec<ModVersionSelection>,
    pub plan: ModrinthPlan,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseConfigurationConflict {
    pub game_version: String,
    pub loader: String,
    pub code: String,
    pub message: String,
    pub projects: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ReverseConfigurationStats {
    pub metadata_requests: usize,
    pub candidate_versions: usize,
    pub targets_considered: usize,
    pub metadata_elapsed_ms: u64,
    pub solve_elapsed_ms: u64,
}

/// Reverse discovery currently adapts public Modrinth client-Mod metadata only.
/// Exact selections are returned for host revalidation before installation.
pub async fn reverse_configure_modrinth(
    project_ids: &[String],
    preferred_game_versions: &[String],
    loaders: &[String],
) -> Result<ReverseConfiguration, BoxError> {
    reverse_configure(
        &mut LiveApi::new()?,
        project_ids,
        preferred_game_versions,
        loaders,
    )
    .await
}

/// Reuse each metadata response within a request, in addition to Broker's
/// public persistent cache and in-flight request merging.
pub(crate) struct CachedApi<'a, A> {
    api: &'a mut A,
    projects: BTreeMap<String, Project>,
    all: BTreeMap<String, Vec<Version>>,
    filtered: BTreeMap<(String, String, String), Vec<Version>>,
    exact: BTreeMap<String, Version>,
    requests: usize,
}
impl<'a, A> CachedApi<'a, A> {
    pub(crate) fn new(api: &'a mut A) -> Self {
        Self {
            api,
            projects: BTreeMap::new(),
            all: BTreeMap::new(),
            filtered: BTreeMap::new(),
            exact: BTreeMap::new(),
            requests: 0,
        }
    }
    pub(crate) fn requests(&self) -> usize {
        self.requests
    }
}
impl<A: Api> CachedApi<'_, A> {
    pub(crate) async fn prefetch_projects(&mut self, projects: &[String]) -> Result<(), BoxError> {
        for (requested, project, versions) in self.api.prefetch_roots(projects).await? {
            self.requests += 2;
            for v in &versions {
                self.exact.insert(v.id.clone(), v.clone());
            }
            self.all.insert(project.id.clone(), versions);
            self.projects.insert(requested, project.clone());
            self.projects.insert(project.id.clone(), project);
        }
        Ok(())
    }
}
impl<A: Api> Api for CachedApi<'_, A> {
    async fn all_versions(&mut self, project: &str) -> Result<Vec<Version>, BoxError> {
        if let Some(v) = self.all.get(project) {
            return Ok(v.clone());
        }
        self.requests += 1;
        let values = self.api.all_versions(project).await?;
        for v in &values {
            self.exact.insert(v.id.clone(), v.clone());
        }
        self.all.insert(project.into(), values.clone());
        Ok(values)
    }
    async fn versions(
        &mut self,
        project: &str,
        mc: &str,
        loader: &str,
    ) -> Result<Vec<Version>, BoxError> {
        if let Some(v) = self.all.get(project) {
            return Ok(v
                .iter()
                .filter(|v| compatible(v, mc, loader))
                .cloned()
                .collect());
        }
        let key = (project.into(), mc.into(), loader.into());
        if let Some(v) = self.filtered.get(&key) {
            return Ok(v.clone());
        }
        self.requests += 1;
        let values = self.api.versions(project, mc, loader).await?;
        for v in &values {
            self.exact.insert(v.id.clone(), v.clone());
        }
        self.filtered.insert(key, values.clone());
        Ok(values)
    }
    async fn version(&mut self, version: &str) -> Result<Version, BoxError> {
        if let Some(v) = self.exact.get(version) {
            return Ok(v.clone());
        }
        self.requests += 1;
        let value = self.api.version(version).await?;
        self.exact.insert(version.into(), value.clone());
        Ok(value)
    }
    async fn project(&mut self, project: &str) -> Result<Project, BoxError> {
        if let Some(v) = self.projects.get(project) {
            return Ok(v.clone());
        }
        self.requests += 1;
        let value = self.api.project(project).await?;
        self.projects.insert(value.id.clone(), value.clone());
        self.projects.insert(project.into(), value.clone());
        Ok(value)
    }
}

async fn reverse_configure(
    api: &mut impl Api,
    requested: &[String],
    preferred: &[String],
    requested_loaders: &[String],
) -> Result<ReverseConfiguration, BoxError> {
    if requested.is_empty() || requested.len() > MAX_PROJECTS {
        return Err(error("Select between 1 and 128 Modrinth projects"));
    }
    if preferred.len() > MAX_TARGETS || requested_loaders.len() > 3 {
        return Err(error("Too many preferred game versions or loaders"));
    }
    let loaders: Vec<_> = if requested_loaders.is_empty() {
        vec!["fabric".into(), "forge".into(), "neoforge".into()]
    } else {
        requested_loaders.to_vec()
    };
    for loader in &loaders {
        target("1.21.1", loader)?;
    }
    for mc in preferred {
        target(mc, &loaders[0])?;
    }
    for id in requested {
        identifier(id)?;
    }
    let started = Instant::now();
    let mut api = CachedApi {
        api,
        projects: BTreeMap::new(),
        all: BTreeMap::new(),
        filtered: BTreeMap::new(),
        exact: BTreeMap::new(),
        requests: 0,
    };
    for (requested_id, project, values) in api.api.prefetch_roots(requested).await? {
        api.requests += 2;
        for v in &values {
            api.exact.insert(v.id.clone(), v.clone());
        }
        api.all.insert(project.id.clone(), values);
        api.projects.insert(requested_id, project.clone());
        api.projects.insert(project.id.clone(), project);
    }
    let mut roots = Vec::new();
    for id in requested {
        identifier(id)?;
        let project = api.project(id).await?;
        supported_client_project(&project)?;
        if roots.contains(&project.id) {
            return Err(error("Duplicate selected Modrinth project"));
        }
        api.all_versions(&project.id).await?;
        roots.push(project.id);
    }
    let mut game_versions = preferred.to_vec();
    if game_versions.is_empty() {
        let mut latest = api.all[&roots[0]].clone();
        latest.sort_by(|a, b| {
            b.date_published
                .cmp(&a.date_published)
                .then(b.id.cmp(&a.id))
        });
        let mut seen = BTreeSet::new();
        for v in latest.iter().filter(|v| v.version_type == "release") {
            for mc in &v.game_versions {
                if target(mc, &loaders[0]).is_ok() && seen.insert(mc.clone()) {
                    game_versions.push(mc.clone());
                }
            }
        }
        // Minecraft's published numeric release identifiers have a known
        // component ordering. Mod display versions remain completely opaque.
        let mut numeric: Vec<_> = game_versions
            .into_iter()
            .filter_map(|v| {
                let components = v
                    .split('.')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()?;
                Some((components, v))
            })
            .collect();
        numeric.sort_by(|a, b| b.0.cmp(&a.0));
        game_versions = numeric.into_iter().map(|(_, v)| v).collect();
    }
    let mut result = ReverseConfiguration {
        choices: Vec::new(),
        conflicts: Vec::new(),
        stats: ReverseConfigurationStats::default(),
        limitations: vec![
            "modrinth_only".into(),
            "client_metadata_only".into(),
            "stable_unpinned_versions_only".into(),
        ],
    };
    let mut seen_targets = BTreeSet::new();
    for mc in &game_versions {
        for loader in &loaders {
            if !seen_targets.insert((mc.clone(), loader.clone())) {
                continue;
            }
            if result.stats.targets_considered >= MAX_TARGETS {
                if !result.limitations.iter().any(|s| s == "target_limit_32") {
                    result.limitations.push("target_limit_32".into());
                }
                break;
            }
            // User preferences are checked even if a root has no matching release;
            // discovery omits targets not advertised by every wanted project.
            let missing: Vec<_> = roots
                .iter()
                .filter(|p| {
                    !api.all[*p].iter().any(|v| {
                        v.project_id == **p
                            && v.version_type == "release"
                            && compatible(v, mc, loader)
                    })
                })
                .cloned()
                .collect();
            if !missing.is_empty() && preferred.is_empty() {
                continue;
            }
            result.stats.targets_considered += 1;
            if !missing.is_empty() {
                result.conflicts.push(ReverseConfigurationConflict {
                    game_version: mc.clone(),
                    loader: loader.clone(),
                    code: "no_compatible_release".into(),
                    message:
                        "Selected projects have no stable release for this game version and loader"
                            .into(),
                    projects: missing,
                });
                continue;
            }
            let mut attempt_stats = solver::AttemptStats::default();
            let attempt = solver::resolve_candidates(
                &mut api,
                &roots,
                &BTreeMap::new(),
                mc,
                loader,
                &mut attempt_stats,
            )
            .await;
            result.stats.candidate_versions += attempt_stats.candidate_count;
            result.stats.solve_elapsed_ms += attempt_stats.solve_elapsed_ms;
            if attempt_stats.truncated
                && !result
                    .limitations
                    .iter()
                    .any(|s| s == "candidate_limit_32_per_project")
            {
                result
                    .limitations
                    .push("candidate_limit_32_per_project".into());
            }
            match attempt {
                Ok(solved) => {
                    result.choices.push(ReverseConfigurationChoice {
                        game_version: mc.clone(),
                        loader: loader.clone(),
                        roots: solved.roots,
                        plan: solved.plan,
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let code = if message.starts_with("candidate_limit:") {
                        "candidate_limit"
                    } else if message.starts_with("unsatisfiable:") {
                        "unsatisfiable"
                    } else if message.starts_with("cancelled_or_limit:") {
                        "cancelled_or_limit"
                    } else {
                        "metadata_unavailable"
                    };
                    result.conflicts.push(ReverseConfigurationConflict {
                        game_version: mc.clone(),
                        loader: loader.clone(),
                        code: code.into(),
                        message,
                        projects: roots.clone(),
                    });
                }
            }
        }
    }
    if result.choices.is_empty() && result.conflicts.is_empty() {
        result.conflicts.push(ReverseConfigurationConflict {
            game_version: String::new(),
            loader: String::new(),
            code: "no_common_target".into(),
            message:
                "Selected projects advertise no common stable game version and supported loader"
                    .into(),
            projects: roots,
        });
    }
    result.stats.metadata_requests = api.requests;
    result.stats.metadata_elapsed_ms =
        (started.elapsed().as_millis() as u64).saturating_sub(result.stats.solve_elapsed_ms);
    Ok(result)
}

#[cfg(test)]
mod tests;

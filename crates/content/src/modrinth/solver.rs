// SPDX-License-Identifier: MIT
//! Finite, bounded Modrinth candidate adaptation. Provider IDs never use display versions.
use super::*;
use crate::dependency_solver::{self, Candidate, PackageKey, SolveRequest, VersionConstraint};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub(super) const MAX_CANDIDATES_PER_PROJECT: usize = 32;
const MAX_CANDIDATES: usize = 4096;

fn key(project: &str) -> PackageKey {
    PackageKey {
        provider: "modrinth".into(),
        project: project.into(),
    }
}
fn constraint(project: &str, version: Option<&str>) -> VersionConstraint {
    match version {
        Some(v) => VersionConstraint::exact(key(project), v.into()),
        None => VersionConstraint::any(key(project)),
    }
}

pub(super) struct Solved {
    pub plan: ModrinthPlan,
    pub roots: Vec<ModVersionSelection>,
}
#[derive(Default)]
pub(crate) struct AttemptStats {
    pub candidate_count: usize,
    pub solve_elapsed_ms: u64,
    pub truncated: bool,
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// Pin existing update roots to their chosen latest releases; reverse discovery
/// deliberately leaves roots free so Resolvo can backtrack their versions too.
pub(crate) async fn collect_candidates(
    api: &mut impl Api,
    roots: &[String],
    pins: &BTreeMap<String, String>,
    mc: &str,
    loader: &str,
    stats: &mut AttemptStats,
) -> Result<Prepared, BoxError> {
    let mut versions = BTreeMap::<String, Version>::new();
    let mut projects = BTreeMap::<String, Project>::new();
    let mut pending_projects: VecDeque<_> = roots.iter().cloned().collect();
    let mut pending_versions = VecDeque::new();
    let mut visited_projects = BTreeSet::new();
    let mut candidates = Vec::new();
    let mut fetched_exact = BTreeMap::new();
    let mut truncated = false;
    // Exact root and required pins may include prereleases, but unconstrained
    // choices retain the existing stable-release policy.
    for (project, pin) in pins {
        let v = api.version(pin).await?;
        if v.id != *pin {
            return Err(error("Modrinth returned a different selected version"));
        }
        version_summary(&v, project, mc, loader)?;
        fetched_exact.insert(v.id.clone(), v.clone());
        pending_versions.push_back(v);
    }
    loop {
        if let Some(project_id) = pending_projects.pop_front() {
            identifier(&project_id)?;
            if !visited_projects.insert(project_id.clone()) {
                continue;
            }
            if visited_projects.len() > MAX_PROJECTS {
                return Err(error(
                    "candidate_limit: Modrinth dependency discovery exceeds 128 projects; reduce the selection",
                ));
            }
            let project = api.project(&project_id).await?;
            if project.id != project_id {
                return Err(error("Modrinth returned a different project"));
            }
            let supported = supported_client_project(&project).is_ok();
            projects.insert(project_id.clone(), project);
            if !supported {
                continue;
            }
            // Fixed update/explicit roots cannot use an alternate release.
            // Avoid discovering metadata for branches that the solver cannot select.
            if pins.contains_key(&project_id) {
                continue;
            }
            let mut choices: Vec<_> = api
                .versions(&project_id, mc, loader)
                .await?
                .into_iter()
                .filter(|v| {
                    v.project_id == project_id
                        && v.version_type == "release"
                        && compatible(v, mc, loader)
                })
                .collect();
            choices.sort_by(|a, b| {
                b.date_published
                    .cmp(&a.date_published)
                    .then(b.id.cmp(&a.id))
            });
            truncated |= choices.len() > MAX_CANDIDATES_PER_PROJECT;
            stats.truncated = truncated;
            pending_versions.extend(choices.into_iter().take(MAX_CANDIDATES_PER_PROJECT));
            continue;
        }
        let Some(version) = pending_versions.pop_front() else {
            break;
        };
        identifier(&version.id)?;
        identifier(&version.project_id)?;
        if versions.contains_key(&version.id) {
            continue;
        }
        if versions.len() >= MAX_CANDIDATES {
            return Err(error(
                "candidate_limit: Modrinth discovery exceeds 4096 candidate versions",
            ));
        }
        if !visited_projects.contains(&version.project_id) {
            pending_projects.push_back(version.project_id.clone());
            pending_versions.push_front(version);
            continue;
        }
        let mut candidate = Candidate {
            package: key(&version.project_id),
            version: version.id.clone(),
            preference: candidates.len(),
            required: Vec::new(),
            incompatible: Vec::new(),
            unavailable: file(&version).err().map(|e| e.to_string()),
        };
        if supported_client_project(&projects[&version.project_id]).is_err() {
            candidate.unavailable = Some("Project is not a supported client mod".into());
        }
        if version.dependencies.len() > MAX_PROJECTS {
            candidate.unavailable = Some("Too many Modrinth dependencies".into());
        } else {
            for dep in &version.dependencies {
                if matches!(dep.dependency_type.as_str(), "optional" | "embedded") {
                    continue;
                }
                if !matches!(dep.dependency_type.as_str(), "required" | "incompatible") {
                    candidate.unavailable = Some("Unknown Modrinth dependency type".into());
                    continue;
                }
                let project_id = if let Some(pin) = &dep.version_id {
                    identifier(pin)?;
                    let pinned = if let Some(v) = fetched_exact.get(pin) {
                        v.clone()
                    } else if let Some(v) = versions.get(pin) {
                        v.clone()
                    } else if let Some(v) = pending_versions.iter().find(|v| &v.id == pin) {
                        v.clone()
                    } else {
                        let value = api.version(pin).await?;
                        fetched_exact.insert(pin.clone(), value.clone());
                        value
                    };
                    if pinned.id != *pin
                        || dep
                            .project_id
                            .as_ref()
                            .is_some_and(|p| p != &pinned.project_id)
                    {
                        candidate.unavailable = Some(format!(
                            "Required pin {pin} has incompatible target metadata"
                        ));
                        continue;
                    }
                    if dep.dependency_type == "required" {
                        if !compatible(&pinned, mc, loader) {
                            candidate.unavailable = Some(format!(
                                "Required pin {pin} has incompatible target metadata"
                            ));
                            continue;
                        }
                        pending_versions.push_back(pinned.clone());
                    }
                    pinned.project_id
                } else if let Some(id) = &dep.project_id {
                    id.clone()
                } else {
                    candidate.unavailable = Some("Dependency has no project or version ID".into());
                    continue;
                };
                identifier(&project_id)?;
                let edge = constraint(&project_id, dep.version_id.as_deref());
                if dep.dependency_type == "required" {
                    pending_projects.push_back(project_id);
                    candidate.required.push(edge);
                } else {
                    candidate.incompatible.push(edge);
                }
            }
        }
        versions.insert(version.id.clone(), version);
        candidates.push(candidate);
        stats.candidate_count = candidates.len();
    }
    let stable_constraint = |project: &str| VersionConstraint {
        package: key(project),
        versions: Some(
            versions
                .values()
                .filter(|v| v.project_id == project && v.version_type == "release")
                .map(|v| v.id.clone())
                .collect(),
        ),
    };
    for candidate in &mut candidates {
        for edge in &mut candidate.required {
            if edge.versions.is_none() {
                *edge = stable_constraint(&edge.package.project);
            }
        }
    }
    // Rank by provider publication metadata, including exact versions discovered
    // through an alternate branch, rather than by graph traversal order.
    let mut ordered: Vec<_> = versions.values().collect();
    ordered.sort_by(|a, b| {
        b.date_published
            .cmp(&a.date_published)
            .then(b.id.cmp(&a.id))
    });
    let preference: BTreeMap<_, _> = ordered
        .iter()
        .enumerate()
        .map(|(i, v)| (v.id.clone(), i))
        .collect();
    for c in &mut candidates {
        c.preference = preference[&c.version];
    }
    stats.candidate_count = candidates.len();
    stats.truncated = truncated;
    let request = SolveRequest {
        candidates,
        roots: roots
            .iter()
            .map(|p| {
                if pins.contains_key(p) {
                    constraint(p, None)
                } else {
                    stable_constraint(p)
                }
            })
            .collect(),
        pins: pins.iter().map(|(p, v)| constraint(p, Some(v))).collect(),
        ..Default::default()
    };
    Ok(Prepared {
        request,
        versions,
        projects,
    })
}

pub(crate) struct Prepared {
    pub request: SolveRequest,
    versions: BTreeMap<String, Version>,
    projects: BTreeMap<String, Project>,
}
impl Prepared {
    pub(crate) fn into_environment_catalog(
        self,
        truncated: bool,
    ) -> crate::environment::catalog::Catalog {
        use crate::environment::catalog::{Artifact, Catalog};
        let artifacts: BTreeMap<_, _> = self
            .versions
            .into_values()
            .filter_map(|version| {
                let file = file(&version).ok()?;
                if !crate::environment::catalog::checked_file(&file) {
                    return None;
                }
                let project = self.projects.get(&version.project_id)?;
                let identity = (key(&version.project_id), version.id.clone());
                Some((
                    identity,
                    Artifact {
                        resolved: ResolvedMod {
                            project_id: version.project_id,
                            version_id: version.id,
                            name: project.title.clone(),
                            version_number: version.version_number,
                            file,
                            required_by: None,
                        },
                        destination: crate::curseforge::PackFileDestination::Mods,
                        restricted: None,
                        known_mod_ids: BTreeSet::new(),
                    },
                ))
            })
            .collect();
        let mut request = self.request;
        for c in &mut request.candidates {
            if !artifacts.contains_key(&(c.package.clone(), c.version.clone()))
                && c.unavailable.is_none()
            {
                c.unavailable = Some("Invalid portable artifact path".into());
            }
        }
        Catalog {
            request,
            artifacts,
            truncated,
            metadata_requests: 0,
        }
    }
}

pub(super) async fn resolve_candidates(
    api: &mut impl Api,
    roots: &[String],
    pins: &BTreeMap<String, String>,
    mc: &str,
    loader: &str,
    stats: &mut AttemptStats,
) -> Result<Solved, BoxError> {
    let Prepared {
        request,
        versions,
        projects,
    } = collect_candidates(api, roots, pins, mc, loader, stats).await?;
    let truncated = stats.truncated;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelOnDrop(cancelled.clone());
    let (selected, elapsed) = tokio::task::spawn_blocking(move || {
        let started = Instant::now();
        let selected =
            dependency_solver::solve_cancellable(request, cancelled, Duration::from_secs(5));
        (selected, started.elapsed().as_millis() as u64)
    })
    .await
    .map_err(|e| error(format!("Dependency solver task failed: {e}")))?;
    stats.solve_elapsed_ms = elapsed;
    let selected = selected.map_err(|e| {
        if truncated && e.code == "unsatisfiable" {
            error(format!("candidate_limit: no solution within the latest 32 stable candidates per project: {}", e.message))
        } else if e.code == "unsatisfiable" {
            error(format!("{}: Conflicting required pins or declared incompatibility. {}", e.code, e.message))
        } else { error(format!("{}: {}", e.code, e.message)) }
    })?;
    let chosen: BTreeMap<_, _> = selected
        .into_iter()
        .map(|c| (c.package.project, c.version))
        .collect();
    let mut parents = BTreeMap::new();
    let mut queue: VecDeque<_> = roots.iter().cloned().collect();
    let mut visited = BTreeSet::new();
    while let Some(project) = queue.pop_front() {
        if !visited.insert(project.clone()) {
            continue;
        }
        let version = &versions[&chosen[&project]];
        for dep in version
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
        {
            let id = dep.project_id.clone().or_else(|| {
                dep.version_id
                    .as_ref()
                    .and_then(|v| versions.get(v).map(|v| v.project_id.clone()))
            });
            if let Some(id) = id {
                if !roots.contains(&id) {
                    parents.entry(id.clone()).or_insert_with(|| project.clone());
                }
                queue.push_back(id);
            }
        }
    }
    let selected_roots = roots
        .iter()
        .map(|p| ModVersionSelection {
            project_id: p.clone(),
            version_id: chosen[p].clone(),
        })
        .collect();
    let mods = chosen
        .iter()
        .map(|(project, id)| {
            let v = &versions[id];
            Ok(ResolvedMod {
                project_id: project.clone(),
                version_id: id.clone(),
                name: projects[project].title.clone(),
                version_number: v.version_number.clone(),
                file: file(v)?,
                required_by: parents.get(project).cloned(),
            })
        })
        .collect::<Result<Vec<_>, BoxError>>()?;
    Ok(Solved {
        plan: ModrinthPlan {
            project_id: roots[0].clone(),
            version_id: chosen[&roots[0]].clone(),
            mods,
        },
        roots: selected_roots,
    })
}

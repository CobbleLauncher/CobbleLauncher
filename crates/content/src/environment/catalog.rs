// SPDX-License-Identifier: MIT
use super::*;
use crate::{
    BoxError, ModrinthFile, ResolvedMod,
    dependency_solver::{
        self, Candidate, PackageKey, SolveRequest, SolverConflict, VersionConstraint,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use unicode_normalization::UnicodeNormalization;

fn portable_keys(name: &str) -> BTreeSet<String> {
    let compatible: String = name.nfkc().collect();
    BTreeSet::from([
        caseless::default_case_fold_str(&compatible).nfc().collect(),
        compatible
            .chars()
            .flat_map(char::to_uppercase)
            .nfc()
            .collect(),
    ])
}

#[derive(Clone)]
pub(crate) struct Artifact {
    pub resolved: ResolvedMod,
    pub destination: curseforge::PackFileDestination,
    pub restricted: Option<curseforge::RestrictedFile>,
    /// Only verified metadata may populate these; upstream display names cannot.
    pub known_mod_ids: BTreeSet<String>,
}
#[derive(Default)]
pub(crate) struct Catalog {
    pub request: SolveRequest,
    pub artifacts: BTreeMap<(PackageKey, String), Artifact>,
    pub truncated: bool,
    pub metadata_requests: usize,
}
impl Catalog {
    pub fn append(&mut self, other: Catalog) {
        self.request.candidates.extend(other.request.candidates);
        self.request.roots.extend(other.request.roots);
        self.request.pins.extend(other.request.pins);
        self.request.preferred.extend(other.request.preferred);
        self.artifacts.extend(other.artifacts);
        self.truncated |= other.truncated;
        self.metadata_requests += other.metadata_requests;
    }
}
pub(crate) fn key(source: EnvironmentSource, project: &str) -> PackageKey {
    PackageKey {
        provider: source.as_str().into(),
        project: project.into(),
    }
}
fn selection(c: &Candidate) -> EnvironmentSelection {
    EnvironmentSelection {
        source: if c.package.provider == "modrinth" {
            EnvironmentSource::Modrinth
        } else {
            EnvironmentSource::Curseforge
        },
        project_id: c.package.project.clone(),
        version_id: c.version.clone(),
    }
}

/// Prevent overwrites and duplicate known content during solving so alternate
/// provider releases can be tried. A name match never establishes a Mod identity.
fn collision_constraints(catalog: &mut Catalog) -> Result<(), BoxError> {
    let mut collisions = BTreeSet::new();
    let mut groups = BTreeMap::<String, Vec<(PackageKey, String)>>::new();
    for (identity, artifact) in &catalog.artifacts {
        let destination = match artifact.destination {
            curseforge::PackFileDestination::Mods => 0,
            curseforge::PackFileDestination::ResourcePack => 1,
            curseforge::PackFileDestination::ShaderPack => 2,
        };
        for path in portable_keys(&artifact.resolved.file.filename) {
            groups
                .entry(format!("path:{destination}:{path}"))
                .or_default()
                .push(identity.clone());
        }
        groups
            .entry(format!(
                "sha1:{}",
                artifact.resolved.file.sha1.to_lowercase()
            ))
            .or_default()
            .push(identity.clone());
        for id in &artifact.known_mod_ids {
            groups
                .entry(format!("mod:{id}"))
                .or_default()
                .push(identity.clone());
        }
    }
    for group in groups.values() {
        for (i, a) in group.iter().enumerate() {
            for b in group.iter().skip(i + 1) {
                if a.0 == b.0 {
                    continue;
                }
                let (a, b) = if a < b { (a, b) } else { (b, a) };
                collisions.insert((a.clone(), b.clone()));
                if collisions.len() > 16_384 {
                    return Err(std::io::Error::other(
                        "candidate_limit: Too many colliding artifact candidates",
                    )
                    .into());
                }
            }
        }
    }
    let mut edges = BTreeMap::<_, Vec<_>>::new();
    for (a, b) in collisions {
        edges
            .entry(a.clone())
            .or_default()
            .push(VersionConstraint::exact(b.0.clone(), b.1.clone()));
        edges
            .entry(b)
            .or_default()
            .push(VersionConstraint::exact(a.0, a.1));
    }
    for candidate in &mut catalog.request.candidates {
        if let Some(extra) = edges.remove(&(candidate.package.clone(), candidate.version.clone())) {
            candidate.incompatible.extend(extra);
        }
    }
    Ok(())
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub(crate) async fn solve_catalog(
    catalog: Catalog,
    roots: &[EnvironmentRoot],
    mc: &str,
    loader: &str,
) -> Result<(EnvironmentPlan, u64, usize), BoxError> {
    solve_catalog_measured(catalog, roots, mc, loader, &mut 0).await
}
pub(crate) async fn solve_catalog_measured(
    mut catalog: Catalog,
    roots: &[EnvironmentRoot],
    mc: &str,
    loader: &str,
    solve_elapsed_ms: &mut u64,
) -> Result<(EnvironmentPlan, u64, usize), BoxError> {
    if catalog.request.candidates.len() > 4096 {
        return Err(
            std::io::Error::other("candidate_limit: Combined graph exceeds4096candidates").into(),
        );
    }
    collision_constraints(&mut catalog)?;
    let count = catalog.request.candidates.len();
    let cancelled = Arc::new(AtomicBool::new(false));
    let _guard = CancelOnDrop(cancelled.clone());
    let request = catalog.request;
    let (result, elapsed) = tokio::task::spawn_blocking(move || {
        let started = Instant::now();
        let result =
            dependency_solver::solve_cancellable(request, cancelled, Duration::from_secs(5));
        (result, started.elapsed().as_millis() as u64)
    })
    .await?;
    *solve_elapsed_ms = elapsed;
    let chosen = result.map_err(|mut conflict| {
        if catalog.truncated && conflict.code == "unsatisfiable" {
            conflict.code = "candidate_limit".into();
        }
        conflict
    })?;
    let selected: BTreeMap<_, _> = chosen
        .iter()
        .map(|c| (c.package.clone(), selection(c)))
        .collect();
    let mut edges = Vec::new();
    for c in &chosen {
        for required in &c.required {
            if let Some(to) = selected.get(&required.package) {
                edges.push(EnvironmentDependencyEdge {
                    from: selection(c),
                    to: to.clone(),
                });
            }
        }
    }
    let exact_roots: Vec<_> = roots
        .iter()
        .map(|r| {
            selected
                .get(&key(r.source, &r.project_id))
                .cloned()
                .ok_or_else(|| std::io::Error::other("Missing selected environment root"))
        })
        .collect::<Result<_, _>>()?;
    let mut mr = Vec::new();
    let mut cf = Vec::new();
    let mut restricted = Vec::new();
    let mut incompatible = BTreeSet::new();
    for c in &chosen {
        let artifact = catalog
            .artifacts
            .get(&(c.package.clone(), c.version.clone()))
            .ok_or_else(|| std::io::Error::other("Missing selected environment artifact"))?;
        let mut resolved = artifact.resolved.clone();
        resolved.required_by = edges
            .iter()
            .find(|e| {
                e.to.project_id == c.package.project && e.to.source.as_str() == c.package.provider
            })
            .map(|e| e.from.project_id.clone());
        if c.package.provider == "modrinth" {
            mr.push(resolved);
        } else {
            cf.push(resolved);
            restricted.extend(artifact.restricted.clone());
            incompatible.extend(
                c.incompatible
                    .iter()
                    .filter(|r| r.package.provider == "curseforge" && r.versions.is_none())
                    .map(|r| r.package.project.clone()),
            );
        }
    }
    let graph = |mut mods: Vec<ResolvedMod>, source| -> Option<crate::ModrinthPlan> {
        if mods.is_empty() {
            return None;
        }
        mods.sort_by(|a, b| a.project_id.cmp(&b.project_id));
        let first = exact_roots.iter().find(|r| r.source == source)?;
        Some(crate::ModrinthPlan {
            project_id: first.project_id.clone(),
            version_id: first.version_id.clone(),
            mods,
        })
    };
    Ok((
        EnvironmentPlan {
            game_version: mc.into(),
            loader: loader.into(),
            roots: exact_roots.clone(),
            modrinth: graph(mr, EnvironmentSource::Modrinth),
            curseforge: graph(cf, EnvironmentSource::Curseforge).map(|graph| {
                curseforge::InstallPlan {
                    graph,
                    restricted,
                    incompatible: incompatible.into_iter().collect(),
                }
            }),
            dependency_edges: edges,
        },
        elapsed,
        count,
    ))
}

pub(crate) fn conflict(
    error: &BoxError,
    mc: &str,
    loader: &str,
    roots: &[EnvironmentRoot],
) -> EnvironmentConflict {
    let (code, message) = if let Some(solver) = error.downcast_ref::<SolverConflict>() {
        (solver.code.clone(), solver.message.clone())
    } else if let Some(curseforge::ApiError::Resolution { code, message, .. }) =
        error.downcast_ref::<curseforge::ApiError>()
    {
        (code.clone(), message.clone())
    } else {
        let text = error.to_string();
        let code = if text.starts_with("candidate_limit:") {
            "candidate_limit"
        } else {
            "metadata_unavailable"
        };
        (code.into(), text)
    };
    EnvironmentConflict {
        game_version: mc.into(),
        loader: loader.into(),
        code,
        message,
        projects: roots
            .iter()
            .map(|r| EnvironmentProject {
                source: r.source,
                project_id: r.project_id.clone(),
            })
            .collect(),
    }
}

pub(crate) fn checked_file(file: &ModrinthFile) -> bool {
    cobble_store::SafeRelativePath::parse(&file.filename).is_ok()
        && !file.filename.is_empty()
        && file.filename.len() <= 240
        && !file.filename.contains(['/', '\\', ':'])
        && !file.filename.chars().any(char::is_control)
        && !file.filename.starts_with('.')
        && file.size > 0
        && file.sha1.len() == 40
        && file.sha1.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests;

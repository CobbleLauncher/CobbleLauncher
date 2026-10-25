// SPDX-License-Identifier: MIT
//! Provider-neutral, finite candidate solving. Version IDs are opaque; adapters
//! supply preference order and explicit sets instead of inventing SemVer.

use resolvo::{
    Candidates, Condition, ConditionId, Dependencies, DependencyProvider, Interner,
    KnownDependencies, NameId, Problem, SolvableId, Solver, SolverCache, StringId,
    UnsolvableOrCancelled, VersionSetId, VersionSetUnionId,
    utils::{Pool, VersionSet},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PackageKey {
    pub provider: String,
    pub project: String,
}
impl fmt::Display for PackageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.project)
    }
}

/// An explicit provider-defined set. `None` accepts every version.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionConstraint {
    pub package: PackageKey,
    pub versions: Option<BTreeSet<String>>,
}
impl VersionConstraint {
    pub fn any(package: PackageKey) -> Self {
        Self {
            package,
            versions: None,
        }
    }
    pub fn exact(package: PackageKey, version: String) -> Self {
        Self {
            package,
            versions: Some(BTreeSet::from([version])),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub package: PackageKey,
    pub version: String,
    /// Lower values are tried first; no ordering is inferred from version text.
    pub preference: usize,
    pub required: Vec<VersionConstraint>,
    pub incompatible: Vec<VersionConstraint>,
    pub unavailable: Option<String>,
}
impl fmt::Display for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.package, self.version)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SolveRequest {
    pub candidates: Vec<Candidate>,
    pub roots: Vec<VersionConstraint>,
    /// Pins constrain a package if selected; they do not install unused packages.
    pub pins: Vec<VersionConstraint>,
    pub preferred: BTreeMap<PackageKey, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolverConflict {
    pub code: String,
    pub message: String,
    pub packages: Vec<PackageKey>,
}
impl fmt::Display for SolverConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for SolverConflict {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Set {
    versions: Option<BTreeSet<String>>,
    complement: bool,
}
impl fmt::Display for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.complement {
            f.write_str("excluding ")?;
        }
        match &self.versions {
            None => f.write_str("all versions"),
            Some(v) => write!(f, "{}", v.iter().cloned().collect::<Vec<_>>().join(" | ")),
        }
    }
}
impl VersionSet for Set {
    type V = Candidate;
}
impl Set {
    fn contains(&self, v: &Candidate) -> bool {
        self.versions
            .as_ref()
            .is_none_or(|set| set.contains(&v.version))
            != self.complement
    }
}
struct Provider {
    pool: Pool<Set, PackageKey>,
    candidates: BTreeMap<PackageKey, Vec<SolvableId>>,
    preferred: BTreeMap<PackageKey, String>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl Provider {
    fn constraint(&self, c: &VersionConstraint, complement: bool) -> VersionSetId {
        let name = self.pool.intern_package_name(c.package.clone());
        self.pool.intern_version_set(
            name,
            Set {
                versions: c.versions.clone(),
                complement,
            },
        )
    }
}
impl Interner for Provider {
    type NameId = NameId;
    type SolvableId = SolvableId;
    fn display_solvable(&self, id: SolvableId) -> impl fmt::Display + '_ {
        &self.pool.resolve_solvable(id).record
    }
    fn display_name(&self, id: NameId) -> impl fmt::Display + '_ {
        self.pool.resolve_package_name(id)
    }
    fn display_version_set(&self, id: VersionSetId) -> impl fmt::Display + '_ {
        self.pool.resolve_version_set(id)
    }
    fn display_string(&self, id: StringId) -> impl fmt::Display + '_ {
        self.pool.resolve_string(id)
    }
    fn version_set_name(&self, id: VersionSetId) -> NameId {
        self.pool.resolve_version_set_package_name(id)
    }
    fn solvable_name(&self, id: SolvableId) -> NameId {
        self.pool.resolve_solvable(id).name
    }
    fn version_sets_in_union(&self, id: VersionSetUnionId) -> impl Iterator<Item = VersionSetId> {
        self.pool.resolve_version_set_union(id)
    }
    fn resolve_condition(&self, id: ConditionId) -> Condition {
        self.pool.resolve_condition(id).clone()
    }
}
impl DependencyProvider for Provider {
    async fn filter_candidates(
        &self,
        candidates: &[SolvableId],
        set: VersionSetId,
        inverse: bool,
    ) -> Vec<SolvableId> {
        let set = self.pool.resolve_version_set(set);
        candidates
            .iter()
            .copied()
            .filter(|id| set.contains(&self.pool.resolve_solvable(*id).record) != inverse)
            .collect()
    }
    async fn get_candidates(&self, name: NameId) -> Option<Candidates> {
        let package = self.pool.resolve_package_name(name);
        let candidates = self.candidates.get(package).cloned().unwrap_or_default();
        let favored = candidates.iter().copied().find(|id| {
            self.preferred.get(package) == Some(&self.pool.resolve_solvable(*id).record.version)
        });
        Some(Candidates {
            candidates,
            favored,
            ..Candidates::default()
        })
    }
    async fn sort_candidates(&self, _: &SolverCache<Self>, ids: &mut [SolvableId]) {
        ids.sort_by_key(|id| {
            let c = &self.pool.resolve_solvable(*id).record;
            (c.preference, c.version.clone())
        });
    }
    async fn get_dependencies(&self, id: SolvableId) -> Dependencies {
        let c = &self.pool.resolve_solvable(id).record;
        if let Some(reason) = &c.unavailable {
            return Dependencies::Unknown(self.pool.intern_string(reason.clone()));
        }
        Dependencies::Known(KnownDependencies {
            requirements: c
                .required
                .iter()
                .map(|c| self.constraint(c, false).into())
                .collect(),
            constrains: c
                .incompatible
                .iter()
                .map(|c| self.constraint(c, true))
                .collect(),
        })
    }
    fn should_cancel_with_value(&self) -> Option<Box<dyn std::any::Any>> {
        (self.cancelled.load(Ordering::Relaxed) || Instant::now() >= self.deadline)
            .then(|| Box::new(()) as Box<dyn std::any::Any>)
    }
}

pub fn solve(request: SolveRequest) -> Result<Vec<Candidate>, SolverConflict> {
    solve_cancellable(
        request,
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(5),
    )
}

/// The timeout covers pure solving; adapters own bounded metadata fetching.
pub fn solve_cancellable(
    request: SolveRequest,
    cancelled: Arc<AtomicBool>,
    timeout: Duration,
) -> Result<Vec<Candidate>, SolverConflict> {
    let packages: Vec<_> = request.roots.iter().map(|c| c.package.clone()).collect();
    let mut provider = Provider {
        pool: Pool::new(),
        candidates: BTreeMap::new(),
        preferred: request.preferred,
        cancelled,
        deadline: Instant::now() + timeout,
    };
    let mut identities = BTreeSet::new();
    for candidate in request.candidates {
        if !identities.insert((candidate.package.clone(), candidate.version.clone())) {
            return Err(SolverConflict {
                code: "invalid_candidates".into(),
                message: "Duplicate provider/project/version identity".into(),
                packages,
            });
        }
        let name = provider.pool.intern_package_name(candidate.package.clone());
        let key = candidate.package.clone();
        let id = provider.pool.intern_solvable(name, candidate);
        provider.candidates.entry(key).or_default().push(id);
    }
    let problem = Problem::new()
        .requirements(
            request
                .roots
                .iter()
                .map(|c| provider.constraint(c, false).into())
                .collect(),
        )
        .constraints(
            request
                .pins
                .iter()
                .map(|c| provider.constraint(c, false))
                .collect(),
        );
    let mut solver = Solver::new(provider);
    match solver.solve(problem) {
        Ok(ids) => Ok(ids
            .into_iter()
            .map(|id| solver.provider().pool.resolve_solvable(id).record.clone())
            .collect()),
        Err(UnsolvableOrCancelled::Unsolvable(conflict)) => Err(SolverConflict {
            code: "unsatisfiable".into(),
            message: conflict.display_user_friendly(&solver).to_string(),
            packages,
        }),
        Err(UnsolvableOrCancelled::Cancelled(_)) => Err(SolverConflict {
            code: "cancelled_or_limit".into(),
            message: "Dependency solving was cancelled or reached its time limit".into(),
            packages,
        }),
    }
}

#[cfg(test)]
mod tests;

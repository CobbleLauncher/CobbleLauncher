// SPDX-License-Identifier: MIT
use super::*;

fn key(provider: &str, project: &str) -> PackageKey {
    PackageKey {
        provider: provider.into(),
        project: project.into(),
    }
}
fn candidate(project: &str, version: &str, preference: usize) -> Candidate {
    Candidate {
        package: key("fixture", project),
        version: version.into(),
        preference,
        required: vec![],
        incompatible: vec![],
        unavailable: None,
    }
}
fn any(project: &str) -> VersionConstraint {
    VersionConstraint::any(key("fixture", project))
}
fn exact(project: &str, version: &str) -> VersionConstraint {
    VersionConstraint::exact(key("fixture", project), version.into())
}

#[test]
fn backtracks_opaque_versions_and_prunes_abandoned_dependencies() {
    let mut newest = candidate("root", "not-semver-new", 0);
    newest.required = vec![any("extra")];
    newest.incompatible = vec![any("other")];
    let old = candidate("root", "not-semver-old", 1);
    let request = SolveRequest {
        candidates: vec![
            newest,
            old,
            candidate("extra", "e", 0),
            candidate("other", "o", 0),
        ],
        roots: vec![any("root"), any("other")],
        ..Default::default()
    };
    let result = solve(request).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|c| c.version == "not-semver-old"));
}

#[test]
fn pins_conflict_and_cycles_deduplicate() {
    let mut a = candidate("a", "a1", 0);
    a.required = vec![exact("b", "b1")];
    let mut b = candidate("b", "b1", 0);
    b.required = vec![any("a")];
    let mut request = SolveRequest {
        candidates: vec![a, b, candidate("b", "b2", 1)],
        roots: vec![any("a")],
        ..Default::default()
    };
    assert_eq!(solve(request.clone()).unwrap().len(), 2);
    request.pins = vec![exact("b", "b2")];
    assert_eq!(solve(request).unwrap_err().code, "unsatisfiable");
}

#[test]
fn source_identity_isolated_and_preferred_version_can_fall_back() {
    let mut one = candidate("same", "one", 0);
    one.package.provider = "one".into();
    let mut two = candidate("same", "two", 0);
    two.package.provider = "two".into();
    let result = solve(SolveRequest {
        candidates: vec![one, two],
        roots: vec![
            VersionConstraint::any(key("one", "same")),
            VersionConstraint::any(key("two", "same")),
        ],
        ..Default::default()
    })
    .unwrap();
    assert_eq!(result.len(), 2);
    let mut request = SolveRequest {
        candidates: vec![candidate("a", "new", 0), candidate("a", "locked", 1)],
        roots: vec![any("a")],
        preferred: BTreeMap::from([(key("fixture", "a"), "locked".into())]),
        ..Default::default()
    };
    assert_eq!(solve(request.clone()).unwrap()[0].version, "locked");
    request.candidates[1].unavailable = Some("Unsupported fixture metadata".into());
    assert_eq!(solve(request).unwrap()[0].version, "new");
}

#[test]
fn cancellation_and_empty_candidate_set_are_distinct() {
    let request = SolveRequest {
        roots: vec![any("missing")],
        ..Default::default()
    };
    assert_eq!(solve(request.clone()).unwrap_err().code, "unsatisfiable");
    assert_eq!(
        solve_cancellable(
            request,
            Arc::new(AtomicBool::new(true)),
            Duration::from_secs(1)
        )
        .unwrap_err()
        .code,
        "cancelled_or_limit"
    );
}

#[test]
fn bounded_fixture_timing_64_projects_512_candidates() {
    let mut candidates = Vec::new();
    for project in 0..64 {
        for release in 0..8 {
            let mut c = candidate(&format!("p{project}"), &format!("opaque{release}"), release);
            if project < 63 {
                c.required.push(any(&format!("p{}", project + 1)));
            }
            if project == 0 && release < 7 {
                c.incompatible.push(any("p63"));
            }
            candidates.push(c);
        }
    }
    let started = Instant::now();
    let result = solve(SolveRequest {
        candidates,
        roots: vec![any("p0"), any("p63")],
        ..Default::default()
    })
    .unwrap();
    eprintln!(
        "synthetic 64 projects / 512 candidates / backtracking: {} us",
        started.elapsed().as_micros()
    );
    assert_eq!(result.len(), 64);
    assert!(
        result
            .iter()
            .any(|c| c.package.project == "p0" && c.version == "opaque7")
    );
}

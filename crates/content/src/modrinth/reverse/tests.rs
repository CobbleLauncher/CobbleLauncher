// SPDX-License-Identifier: MIT
use super::super::tests::{Fixture, version};
use super::*;
use serde_json::json;

#[test]
fn wanted_roots_backtrack_and_target_order_reuses_metadata() {
    let new = version(
        "Root",
        "Newest",
        "2026-03-01",
        json!([{"project_id":"Other","dependency_type":"incompatible"}]),
    );
    let old = version("Root", "Older", "2026-02-01", json!([]));
    let other = version("Other", "OtherV", "2026-02-01", json!([]));
    let mut api = Fixture {
        versions: vec![new, old, other],
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(reverse_configure(
            &mut api,
            &["Root".into(), "Other".into()],
            &["1.20.1".into(), "1.21.1".into()],
            &["fabric".into()],
        ))
        .unwrap();
    assert_eq!(result.choices.len(), 1);
    assert_eq!(result.choices[0].game_version, "1.21.1");
    assert_eq!(result.choices[0].roots[0].version_id, "Older");
    assert_eq!(result.conflicts[0].code, "no_compatible_release");
    assert_eq!(result.stats.metadata_requests, 4); // One project and all-versions fetch per root.
    assert_eq!(result.stats.candidate_versions, 3);
    assert_eq!(result.stats.targets_considered, 2);
}

#[test]
fn advertised_target_discovery_and_candidate_bounds_are_honest() {
    let mut versions = vec![version("Root", "OlderCompatible", "2020-01-01", json!([]))];
    for i in 0..33 {
        versions.push(version(
            "Root",
            &format!("New{i}"),
            &format!("2026-01-{:02}", i + 1),
            json!([{"project_id":"Other","dependency_type":"incompatible"}]),
        ));
    }
    versions.push(version("Other", "OtherV", "2026-01-01", json!([])));
    let mut api = Fixture { versions };
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(reverse_configure(
            &mut api,
            &["Root".into(), "Other".into()],
            &[],
            &["fabric".into()],
        ))
        .unwrap();
    assert!(result.choices.is_empty());
    assert_eq!(result.conflicts[0].code, "candidate_limit");
    assert!(result.conflicts[0].message.contains("latest 32"));
}

#[test]
fn server_only_and_unsupported_loader_are_rejected() {
    let mut api = Fixture {
        versions: vec![version("Server", "ServerV", "2026-01-01", json!([]))],
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    assert!(
        runtime
            .block_on(reverse_configure(
                &mut api,
                &["Server".into()],
                &[],
                &["fabric".into()]
            ))
            .is_err()
    );
    assert!(
        runtime
            .block_on(reverse_configure(
                &mut api,
                &["Server".into()],
                &[],
                &["quilt".into()]
            ))
            .is_err()
    );
}

#[test]
fn numeric_game_releases_rank_before_recent_old_line_publications() {
    let mut old_line = version("Root", "OldLine", "2026-03-01", json!([]));
    old_line.game_versions = vec!["1.20.1".into()];
    let modern = version("Root", "Modern", "2026-02-01", json!([]));
    let mut api = Fixture {
        versions: vec![old_line, modern],
    };
    let result = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(reverse_configure(
            &mut api,
            &["Root".into()],
            &[],
            &["fabric".into()],
        ))
        .unwrap();
    assert_eq!(
        result
            .choices
            .iter()
            .map(|c| c.game_version.as_str())
            .collect::<Vec<_>>(),
        vec!["1.21.1", "1.20.1"]
    );
    assert_eq!(result.stats.metadata_requests, 2);
}

#[test]
fn repeated_metadata_lookups_reuse_in_request_cache() {
    let mut source = Fixture {
        versions: vec![version("Root", "RootV", "2026-01-01", json!([]))],
    };
    let mut api = CachedApi {
        api: &mut source,
        projects: BTreeMap::new(),
        all: BTreeMap::new(),
        filtered: BTreeMap::new(),
        exact: BTreeMap::new(),
        requests: 0,
    };
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for _ in 0..3 {
                api.project("Root").await.unwrap();
                api.all_versions("Root").await.unwrap();
                api.versions("Root", "1.21.1", "fabric").await.unwrap();
                api.version("RootV").await.unwrap();
            }
        });
    assert_eq!(api.requests, 2);
}

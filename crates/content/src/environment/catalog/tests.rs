// SPDX-License-Identifier: MIT
use super::*;

fn add(
    catalog: &mut Catalog,
    source: EnvironmentSource,
    project: &str,
    version: &str,
    preference: usize,
    filename: &str,
    digest: u32,
) -> PackageKey {
    let package = key(source, project);
    catalog.request.candidates.push(Candidate {
        package: package.clone(),
        version: version.into(),
        preference,
        required: vec![],
        incompatible: vec![],
        unavailable: None,
    });
    catalog.artifacts.insert(
        (package.clone(), version.into()),
        Artifact {
            resolved: ResolvedMod {
                project_id: project.into(),
                version_id: version.into(),
                name: "Same display title".into(),
                version_number: "not-semver".into(),
                file: ModrinthFile {
                    url: if source == EnvironmentSource::Modrinth {
                        "https://cdn.modrinth.com/data/x.jar".into()
                    } else {
                        "https://edge.forgecdn.net/files/1/x.jar".into()
                    },
                    filename: filename.into(),
                    size: 42,
                    sha1: format!("{digest:040x}"),
                    sha512: None,
                },
                required_by: None,
            },
            destination: curseforge::PackFileDestination::Mods,
            restricted: None,
            known_mod_ids: BTreeSet::new(),
        },
    );
    package
}
fn root(source: EnvironmentSource, project: &str, version: Option<&str>) -> EnvironmentRoot {
    EnvironmentRoot {
        source,
        project_id: project.into(),
        version_id: version.map(str::to_owned),
    }
}
fn setup(catalog: &mut Catalog, roots: &[EnvironmentRoot]) {
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
}
#[test]
fn environment_solver_mixed_paths_backtrack_and_exact_pins_conflict() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for pinned in [false, true] {
                let mut catalog = Catalog::default();
                add(
                    &mut catalog,
                    EnvironmentSource::Modrinth,
                    "1",
                    "new",
                    0,
                    "Shared.jar",
                    1,
                );
                add(
                    &mut catalog,
                    EnvironmentSource::Modrinth,
                    "1",
                    "older",
                    1,
                    "different.jar",
                    2,
                );
                add(
                    &mut catalog,
                    EnvironmentSource::Curseforge,
                    "1",
                    "10",
                    0,
                    "shared.jar",
                    3,
                );
                let roots = vec![
                    root(EnvironmentSource::Modrinth, "1", pinned.then_some("new")),
                    root(EnvironmentSource::Curseforge, "1", Some("10")),
                ];
                setup(&mut catalog, &roots);
                let result = solve_catalog(catalog, &roots, "1.21.1", "fabric").await;
                if pinned {
                    assert_eq!(
                        result
                            .err()
                            .unwrap()
                            .downcast_ref::<SolverConflict>()
                            .unwrap()
                            .code,
                        "unsatisfiable"
                    );
                } else {
                    let plan = result.unwrap().0;
                    assert_eq!(plan.roots[0].version_id, "older");
                    assert_eq!(plan.roots.len(), 2);
                    assert!(plan.modrinth.is_some() && plan.curseforge.is_some());
                }
            }
        });
}
#[test]
fn environment_solver_known_content_and_mod_id_collisions_are_not_name_matching() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            for mode in 0..3 {
                let mut catalog = Catalog::default();
                let a = add(
                    &mut catalog,
                    EnvironmentSource::Modrinth,
                    "a",
                    "one",
                    0,
                    "one.jar",
                    1,
                );
                let b = add(
                    &mut catalog,
                    EnvironmentSource::Curseforge,
                    "2",
                    "20",
                    0,
                    "two.jar",
                    if mode == 1 { 1 } else { 2 },
                );
                if mode == 2 {
                    catalog
                        .artifacts
                        .get_mut(&(a, "one".into()))
                        .unwrap()
                        .known_mod_ids
                        .insert("verified_game_id".into());
                    catalog
                        .artifacts
                        .get_mut(&(b, "20".into()))
                        .unwrap()
                        .known_mod_ids
                        .insert("verified_game_id".into());
                }
                let roots = vec![
                    root(EnvironmentSource::Modrinth, "a", None),
                    root(EnvironmentSource::Curseforge, "2", None),
                ];
                setup(&mut catalog, &roots);
                let result = solve_catalog(catalog, &roots, "1.21.1", "fabric").await;
                assert_eq!(result.is_ok(), mode == 0);
            }
        });
}
#[test]
fn environment_solver_required_cycle_preserves_all_exact_edges() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let mut catalog = Catalog::default();
            let a = add(
                &mut catalog,
                EnvironmentSource::Modrinth,
                "a",
                "a1",
                0,
                "a.jar",
                1,
            );
            let b = add(
                &mut catalog,
                EnvironmentSource::Modrinth,
                "b",
                "b1",
                0,
                "b.jar",
                2,
            );
            catalog.request.candidates[0]
                .required
                .push(VersionConstraint::any(b));
            catalog.request.candidates[1]
                .required
                .push(VersionConstraint::any(a));
            let roots = vec![root(EnvironmentSource::Modrinth, "a", Some("a1"))];
            setup(&mut catalog, &roots);
            let plan = solve_catalog(catalog, &roots, "1.21.1", "fabric")
                .await
                .unwrap()
                .0;
            assert_eq!(plan.dependency_edges.len(), 2);
            assert_eq!(plan.modrinth.unwrap().mods.len(), 2);
        });
}
#[test]
fn environment_solver_portable_paths_handle_unicode_aliases_and_devices() {
    assert!(!portable_keys("Straße.jar").is_disjoint(&portable_keys("STRASSE.jar")));
    let file = ModrinthFile {
        url: String::new(),
        filename: "CON.jar".into(),
        size: 1,
        sha1: "a".repeat(40),
        sha512: None,
    };
    assert!(!checked_file(&file));
    assert!(!checked_file(&ModrinthFile {
        filename: "../escape.jar".into(),
        ..file.clone()
    }));
}

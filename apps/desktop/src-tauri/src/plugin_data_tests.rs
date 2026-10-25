// SPDX-License-Identifier: MIT
use super::*;
use serde_json::json;

fn scope(game: Option<&str>, world: Option<&str>) -> Scope {
    Scope {
        game_id: game.map(str::to_owned),
        world_id: world.map(str::to_owned),
        server_id: None,
    }
}

fn save(
    directory: &Path,
    plugin: &str,
    scope: Scope,
    key: &str,
    value: Value,
    expected: Option<String>,
) -> Result<Document, DataError> {
    write(
        directory,
        plugin,
        WriteRequest {
            scope,
            key: key.into(),
            value,
            expected_revision: expected,
        },
        &AtomicBool::new(false),
    )
}

#[test]
fn plugin_game_world_namespaces_survive_reload_and_never_become_paths() {
    let directory = tempfile::tempdir().unwrap();
    let scopes = [
        scope(None, None),
        scope(Some("game-a"), None),
        scope(Some("game-a"), Some("world/a")),
        scope(Some("game-a"), Some("world-b")),
        scope(Some("game-b"), Some("world/a")),
    ];
    for (index, scoped) in scopes.iter().enumerate() {
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            "points",
            json!({"index":index}),
            None,
        )
        .unwrap();
    }
    for (index, scoped) in scopes.iter().enumerate() {
        assert_eq!(
            read(directory.path(), "waypoints", scoped, "points")
                .unwrap()
                .value,
            json!({"index":index})
        );
        assert_eq!(
            read(directory.path(), "other-plugin", scoped, "points").unwrap(),
            Document::default()
        );
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    assert!(directory.path().join("waypoints.json").is_file());
    assert!(!directory.path().join("game-a").exists());
    assert!(authorize_scope(Some("game-a"), &scope(Some("game-b"), None)).is_err());
    assert_eq!(
        authorize_scope(None, &scope(None, Some("world"))).unwrap_err(),
        "PLUGIN_DATA_INVALID_SCOPE"
    );
    assert!(authorize_scope(Some("game-a"), &scope(None, None)).is_ok());
}

#[test]
fn revision_conflicts_preserve_data_and_delete_requires_current_revision() {
    let directory = tempfile::tempdir().unwrap();
    let scoped = Scope::default();
    let first = save(
        directory.path(),
        "waypoints",
        scoped.clone(),
        "points",
        json!([1]),
        None,
    )
    .unwrap();
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            "points",
            json!([2]),
            None
        )
        .unwrap_err(),
        DataError::Conflict
    );
    let second = save(
        directory.path(),
        "waypoints",
        scoped.clone(),
        "points",
        json!([1]),
        first.revision.clone(),
    )
    .unwrap();
    assert_ne!(first.revision, second.revision);
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            "points",
            json!([3]),
            first.revision.clone()
        )
        .unwrap_err(),
        DataError::Conflict
    );
    let stale = RemoveRequest {
        scope: scoped.clone(),
        key: "points".into(),
        expected_revision: first.revision,
    };
    assert_eq!(
        remove(
            directory.path(),
            "waypoints",
            stale,
            &AtomicBool::new(false)
        )
        .unwrap_err(),
        DataError::Conflict
    );
    assert_eq!(
        read(directory.path(), "waypoints", &scoped, "points").unwrap(),
        second
    );
    remove(
        directory.path(),
        "waypoints",
        RemoveRequest {
            scope: scoped.clone(),
            key: "points".into(),
            expected_revision: second.revision.clone(),
        },
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        read(directory.path(), "waypoints", &scoped, "points").unwrap(),
        Document::default()
    );
    let recreated = save(
        directory.path(),
        "waypoints",
        scoped.clone(),
        "points",
        Value::Null,
        None,
    )
    .unwrap();
    assert_ne!(recreated.revision, second.revision);
    assert_eq!(recreated.value, Value::Null);
    assert!(recreated.revision.is_some());
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped,
            "points",
            json!([4]),
            second.revision
        )
        .unwrap_err(),
        DataError::Conflict
    );
}

#[test]
fn value_and_plugin_quotas_fail_without_replacing_previous_file() {
    let directory = tempfile::tempdir().unwrap();
    let scoped = Scope::default();
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            "huge",
            Value::String("x".repeat(MAX_DOCUMENT)),
            None
        )
        .unwrap_err(),
        DataError::TooLarge
    );
    let value = Value::String("x".repeat(MAX_DOCUMENT - 2));
    for index in 0..31 {
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            &format!("key_{index}"),
            value.clone(),
            None,
        )
        .unwrap();
    }
    let before = fs::read(directory.path().join("waypoints.json")).unwrap();
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped.clone(),
            "overquota",
            value.clone(),
            None
        )
        .unwrap_err(),
        DataError::Quota
    );
    assert_eq!(
        fs::read(directory.path().join("waypoints.json")).unwrap(),
        before
    );
    assert_eq!(
        read(directory.path(), "waypoints", &scoped, "key_0")
            .unwrap()
            .value,
        value
    );
}

#[test]
fn malformed_inputs_cannot_bypass_revision_or_permission_checks() {
    let request = json!({"scope":{},"key":"points","value":null});
    assert!(serde_json::from_value::<WriteRequest>(request).is_err());
    assert!(serde_json::from_value::<RemoveRequest>(json!({"scope":{},"key":"points"})).is_err());
    assert!(
        serde_json::from_value::<ReadRequest>(
            json!({"scope":{},"key":"points","pluginId":"foreign"})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<ReadRequest>(json!({"scope":{"path":"arbitrary"},"key":"points"}))
            .is_err()
    );
    for key in ["", "bad/key", "bad.key", "你好", &"x".repeat(65)] {
        assert_eq!(
            slot(&Scope::default(), key).unwrap_err(),
            DataError::InvalidKey
        );
    }
    assert_eq!(
        revision(Some("not-a-revision")).unwrap_err(),
        DataError::InvalidRevision
    );
    let mut manifest: PluginManifest = serde_json::from_str(include_str!(
        "../../../../examples/seed-compass/plugin.json"
    ))
    .unwrap();
    manifest.permissions = Some(vec![]);
    assert_eq!(
        permitted(&manifest).unwrap_err(),
        "PLUGIN_PERMISSION_DENIED"
    );
    manifest
        .permissions
        .get_or_insert_default()
        .push(PermissionNode::PrivateStorage {});
    assert!(permitted(&manifest).is_ok());
}

#[test]
fn cancelled_view_cannot_commit_and_corrupt_data_is_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let scoped = Scope::default();
    let request = WriteRequest {
        scope: scoped.clone(),
        key: "points".into(),
        value: json!([1]),
        expected_revision: None,
    };
    assert_eq!(
        write(
            directory.path(),
            "waypoints",
            request,
            &AtomicBool::new(true)
        )
        .unwrap_err(),
        DataError::Revoked
    );
    assert!(!directory.path().join("waypoints.json").exists());
    fs::write(
        directory.path().join("waypoints.json"),
        b"invalid original bytes",
    )
    .unwrap();
    assert_eq!(
        read(directory.path(), "waypoints", &scoped, "points").unwrap_err(),
        DataError::Corrupt
    );
    assert_eq!(
        save(
            directory.path(),
            "waypoints",
            scoped,
            "points",
            json!([]),
            None
        )
        .unwrap_err(),
        DataError::Corrupt
    );
    assert_eq!(
        fs::read(directory.path().join("waypoints.json")).unwrap(),
        b"invalid original bytes"
    );
}

#[test]
fn simultaneous_native_writers_cannot_overwrite_the_same_revision() {
    let directory = tempfile::tempdir().unwrap();
    let first = save(
        directory.path(),
        "waypoints",
        Scope::default(),
        "points",
        json!([]),
        None,
    )
    .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|index| {
            let directory = directory.path().to_owned();
            let expected = first.revision.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                let _guard = WRITES.lock().unwrap();
                save(
                    &directory,
                    "waypoints",
                    Scope::default(),
                    "points",
                    json!([index]),
                    expected,
                )
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(DataError::Conflict)))
            .count(),
        1
    );
}

#[test]
fn server_scopes_are_exclusive_and_old_scope_serialization_is_unchanged() {
    let old = scope(Some("g"), Some("w"));
    assert_eq!(
        serde_json::to_string(&old).unwrap(),
        r#"{"gameId":"g","worldId":"w"}"#
    );
    let old_bytes = br#"[{"gameId":"g","worldId":"w"},"notes"]"#;
    assert_eq!(
        slot(&old, "notes").unwrap(),
        cobble_plugin_host::digest(old_bytes)
    );
    let server = Scope {
        game_id: Some("g".into()),
        server_id: Some("a".repeat(64)),
        ..Scope::default()
    };
    assert!(authorize_scope(Some("g"), &server).is_ok());
    assert!(authorize_scope(Some("other"), &server).is_err());
    assert!(
        authorize_scope(
            None,
            &Scope {
                world_id: Some("w".into()),
                ..server.clone()
            }
        )
        .is_err()
    );
    assert!(
        authorize_scope(
            None,
            &Scope {
                game_id: None,
                ..server.clone()
            }
        )
        .is_err()
    );
    assert!(
        authorize_scope(
            None,
            &Scope {
                server_id: Some("example.org".into()),
                ..server.clone()
            }
        )
        .is_err()
    );
    assert_ne!(
        slot(&server, "notes").unwrap(),
        slot(&scope(Some("g"), None), "notes").unwrap()
    );
    let directory = tempfile::tempdir().unwrap();
    save(
        directory.path(),
        "notes",
        server.clone(),
        "notes",
        json!([1]),
        None,
    )
    .unwrap();
    assert_eq!(
        read(directory.path(), "notes", &server, "notes")
            .unwrap()
            .value,
        json!([1])
    );
}

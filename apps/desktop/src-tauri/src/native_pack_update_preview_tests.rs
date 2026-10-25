// SPDX-License-Identifier: MIT
use super::*;
fn fixture_pack(path: &Path, version: &str, config: &[u8]) {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("modrinth.index.json", options).unwrap();
    zip.write_all(serde_json::to_string(&serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":version,"name":"Fixture","files":[],"dependencies":{"minecraft":"1.21.1"}})).unwrap().as_bytes()).unwrap();
    zip.start_file("overrides/config/settings.txt", options)
        .unwrap();
    zip.write_all(config).unwrap();
    zip.finish().unwrap();
}
#[test]
fn actual_local_archives_compare_with_user_state_and_missing_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("old.mrpack");
    let target = temp.path().join("new.mrpack");
    fixture_pack(&source, "v1", b"old pack");
    fixture_pack(&target, "v2", b"new pack");
    let game_dir = temp.path().join("game");
    fs::create_dir_all(game_dir.join("config")).unwrap();
    fs::create_dir_all(game_dir.join("mods")).unwrap();
    fs::write(game_dir.join("config/settings.txt"), b"player edit").unwrap();
    fs::write(game_dir.join("mods/player.jar"), b"player mod").unwrap();
    let original_hash = format!("{:x}", Sha256::digest(fs::read(&source).unwrap()));
    fs::write(game_dir.join(".cobble-pack.json"),serde_json::to_vec(&serde_json::json!({"name":"Fixture","version_id":"v1","archive_sha256":original_hash,"selected_optional":[],"complete":true})).unwrap()).unwrap();
    let game:Game=serde_json::from_value(serde_json::json!({"id":"fixture","name":"Fixture","version":"1.21.1","directory":game_dir,"status":"installed","pack_source_id":"a1","pack_name":"Fixture","pack_version":"v1"})).unwrap();
    let compared = compare_archives(&game, &source, &target).unwrap();
    assert!(compared.baseline_available);
    assert_eq!(compared.plan.version_changed.len(), 1);
    assert_eq!(
        compared.plan.version_changed[0].local_state,
        domain::PackLocalState::Modified
    );
    let (files, added) =
        super::super::pack_update_copy::local_candidates(&game, &compared.plan).unwrap();
    assert_eq!(added, vec!["mods/player.jar"]);
    assert!(files.contains_key("config/settings.txt"));
    assert_eq!(
        fs::read(game.directory.join("config/settings.txt")).unwrap(),
        b"player edit"
    );
    fs::remove_file(source).unwrap();
    let without = compare_archives(&game, &temp.path().join("old.mrpack"), &target).unwrap();
    assert_eq!(without.baseline_status, "missing");
    assert!(!without.baseline_available);
    assert_eq!(without.plan.added.len(), 1);
}
#[test]
fn retained_archive_digest_is_checked_before_parsing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("original.mrpack");
    fs::write(&path, b"changed original").unwrap();
    assert_eq!(
        archive(&path, Some(&"0".repeat(64)), &[]).err().unwrap(),
        "PACK_UPDATE_SOURCE_CHANGED"
    );
    assert_eq!(fs::read(&path).unwrap(), b"changed original");
}
#[test]
fn scoped_observations_preserve_missing_disabled_and_budget_unknown_states() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("mods")).unwrap();
    fs::write(temp.path().join("mods/a.jar"), b"local bytes").unwrap();
    fs::write(temp.path().join("mods/off.jar.disabled"), b"disabled").unwrap();
    let root = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
    let mut budget = 64;
    let observed = observation(&root, "mods/a.jar", &mut budget).unwrap();
    assert!(observed.exists);
    assert_eq!(
        observed.sha512,
        Some(format!("{:x}", Sha512::digest(b"local bytes")))
    );
    assert!(
        !observation(&root, "mods/missing.jar", &mut budget)
            .unwrap()
            .exists
    );
    assert!(observation(&root, "mods/off.jar", &mut budget).is_none());
    assert!(observation(&root, "mods/a.jar", &mut 0).is_none());
    assert_eq!(
        fs::read(temp.path().join("mods/a.jar")).unwrap(),
        b"local bytes"
    );
}
#[test]
fn absent_pack_provenance_is_explicitly_unsupported() {
    let temp = tempfile::tempdir().unwrap();
    let game:Game=serde_json::from_value(serde_json::json!({"id":"fixture","name":"Fixture","version":"1.21.1","directory":temp.path(),"status":"installed","error":null,"pack_name":"Display label is not provenance"})).unwrap();
    let result = context(&game);
    assert!(!result.supported);
    assert_eq!(result.reason, Some("not_managed_pack"));
    assert!(!result.online_versions);
}
#[cfg(windows)]
#[test]
fn observed_linked_parent_never_reads_foreign_files() {
    use std::os::windows::process::CommandExt;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.cfg"), b"private").unwrap();
    let link = temp.path().join("config");
    let result = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(result.status.success());
    let root = Dir::open_ambient_dir(temp.path(), cap_std::ambient_authority()).unwrap();
    assert!(observation(&root, "config/secret.cfg", &mut 64).is_none());
    fs::remove_dir(link).unwrap();
    assert_eq!(
        fs::read(outside.path().join("secret.cfg")).unwrap(),
        b"private"
    );
}

// SPDX-License-Identifier: MIT
use super::*;
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

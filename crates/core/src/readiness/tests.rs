// SPDX-License-Identifier: MIT
use super::*;
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cobble-readiness-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Temp, String) {
    let temp = Temp::new();
    let root = &temp.0;
    std::fs::create_dir_all(root.join("versions/fixture")).unwrap();
    std::fs::create_dir_all(root.join("assets/indexes")).unwrap();
    let asset = format!("{:x}", Sha1::digest(b"asset"));
    let index =
        serde_json::to_vec(&serde_json::json!({"objects":{"test":{"hash":asset,"size":5}}}))
            .unwrap();
    std::fs::write(root.join("assets/indexes/fixture.json"), &index).unwrap();
    std::fs::write(root.join("versions/fixture/fixture.jar"), b"client").unwrap();
    std::fs::write(
        root.join(".cobble-install.json"),
        br#"{"version":"fixture","complete":true,"client_only":false}"#,
    )
    .unwrap();
    let metadata = serde_json::json!({"id":"fixture","mainClass":"test.Main","javaVersion":{"majorVersion":21},
        "assetIndex":{"id":"fixture","url":"https://piston-meta.mojang.com/fixture","sha1":format!("{:x}",Sha1::digest(&index)),"size":index.len()},
        "downloads":{"client":{"url":"https://piston-data.mojang.com/fixture.jar","sha1":format!("{:x}",Sha1::digest(b"client")),"size":6}},
        "libraries":[],"arguments":{"jvm":[],"game":[]}});
    std::fs::write(
        root.join("versions/fixture/fixture.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    (temp, asset)
}
fn control() -> crate::OperationControl {
    crate::OperationControl::new(std::sync::Arc::new(|_| {}))
}
fn add_asset(root: &Path, hash: &str) {
    let p = root.join(format!("assets/objects/{}/{hash}", &hash[..2]));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"asset").unwrap();
}
#[test]
fn readiness_finds_missing_asset_objects_locally_and_becomes_ready_only_after_exact_bytes() {
    let (temp, hash) = fixture();
    let report = inspect_local_readiness("fixture", &temp.0, None, None, control()).unwrap();
    assert!(!report.ready);
    assert_eq!(report.required_java, Some(21));
    assert_eq!(report.missing_files, 1);
    assert_eq!(report.issues[0].category, "assets");
    add_asset(&temp.0, &hash);
    assert!(
        inspect_local_readiness("fixture", &temp.0, None, None, control())
            .unwrap()
            .ready
    );
}
#[test]
fn readiness_preserves_disabled_content_and_reports_missing_or_changed_without_mutation() {
    let (temp, hash) = fixture();
    add_asset(&temp.0, &hash);
    std::fs::create_dir(temp.0.join("mods")).unwrap();
    let digest = format!("{:x}", Sha1::digest(b"selected"));
    std::fs::write(temp.0.join(".cobble-modrinth.json"),serde_json::to_vec(&serde_json::json!({"project":{"version_id":"exact","filename":"selected.jar","sha1":digest}})).unwrap()).unwrap();
    let file = temp.0.join("mods/selected.jar.disabled");
    std::fs::write(&file, b"selected").unwrap();
    let ready = inspect_local_readiness("fixture", &temp.0, None, None, control()).unwrap();
    assert!(ready.ready);
    std::fs::write(&file, b"modified").unwrap();
    let changed = inspect_local_readiness("fixture", &temp.0, None, None, control()).unwrap();
    assert_eq!(changed.changed_files, 1);
    assert!(!changed.issues[0].repairable);
    assert_eq!(std::fs::read(&file).unwrap(), b"modified");
    std::fs::remove_file(file).unwrap();
    let missing = inspect_local_readiness("fixture", &temp.0, None, None, control()).unwrap();
    assert_eq!(missing.missing_files, 1);
    assert_eq!(missing.content_revision, ready.content_revision);
}
#[test]
fn readiness_incomplete_marker_and_cancelled_work_cannot_claim_ready() {
    let (temp, hash) = fixture();
    add_asset(&temp.0, &hash);
    std::fs::write(
        temp.0.join(".cobble-install.json"),
        br#"{"version":"fixture","complete":false,"client_only":false}"#,
    )
    .unwrap();
    let report = inspect_local_readiness("fixture", &temp.0, None, None, control()).unwrap();
    assert!(!report.ready);
    assert!(report.unknown > 0);
    let cancelled = control();
    cancelled.cancel();
    assert!(inspect_local_readiness("fixture", &temp.0, None, None, cancelled).is_err());
}
#[cfg(unix)]
#[test]
fn readiness_never_follows_a_managed_path_parent_symlink() {
    let root = Temp::new();
    let outside = Temp::new();
    std::fs::write(outside.0.join("private.jar"), b"private").unwrap();
    std::os::unix::fs::symlink(&outside.0, root.0.join("libraries")).unwrap();
    let dir = Dir::open_ambient_dir(&root.0, cap_std::ambient_authority()).unwrap();
    assert!(open_scoped(&dir, "libraries/private.jar").is_err());
}

// SPDX-License-Identifier: MIT
use super::*;
fn file(path: &str, hash: char) -> PackManifestFile {
    PackManifestFile {
        identity: path.into(),
        version_id: None,
        path: path.into(),
        size_bytes: 4,
        sha1: Some(hash.to_string().repeat(40)),
        sha512: None,
    }
}
fn pack(files: Vec<PackManifestFile>) -> PackManifestSnapshot {
    PackManifestSnapshot {
        source: PackSource {
            provider: PackProvider::Mrpack,
            project_id: None,
        },
        version_id: "original-1".into(),
        minecraft_version: "1.21.1".into(),
        loader_kind: Some("fabric".into()),
        loader_version: Some("0.16".into()),
        files,
    }
}
fn observed(file: &PackManifestFile) -> ObservedPackFile {
    ObservedPackFile {
        path: file.path.clone(),
        exists: true,
        size_bytes: file.size_bytes,
        sha1: file.sha1.clone(),
        sha512: file.sha512.clone(),
    }
}
#[test]
fn added_removed_changed_preserve_worlds_configs_and_modified_files() {
    let old = pack(vec![
        file("mods/changed.jar", 'a'),
        file("mods/removed.jar", 'b'),
        file("config/example.toml", 'c'),
        file("saves/World/level.dat", 'd'),
    ]);
    let mut target = pack(vec![
        file("mods/changed.jar", 'b'),
        file("mods/added.jar", 'e'),
        file("config/example.toml", 'd'),
        file("saves/World/level.dat", 'e'),
    ]);
    target.version_id = "original-2".into();
    target.loader_version = Some("0.17".into());
    let mut observations: Vec<_> = old.files.iter().map(observed).collect();
    observations[0].sha1 = Some("f".repeat(40));
    observations.push(ObservedPackFile {
        path: "mods/added.jar".into(),
        exists: false,
        size_bytes: 0,
        sha1: None,
        sha512: None,
    });
    let preview = preview_pack_update(Some(&old), &target, &observations).unwrap();
    assert_eq!(
        (
            preview.added.len(),
            preview.removed.len(),
            preview.version_changed.len()
        ),
        (1, 1, 3)
    );
    assert!(preview.runtime_changed);
    assert_eq!(preview.identity_relation, PackIdentityRelation::Unknown);
    assert_eq!(preview.added[0].local_state, PackLocalState::Missing);
    assert!(!preview.added[0].preserve);
    assert!(preview.version_changed.iter().all(|row| row.preserve));
    assert_eq!(
        preview.version_changed[1].local_state,
        PackLocalState::Modified
    );
    assert!(!preview.removed[0].preserve);
}
#[test]
fn missing_provenance_and_unknown_observations_never_become_verified() {
    let old = pack(vec![file("mods/a.jar", 'a')]);
    assert_eq!(
        preview_pack_update(None, &old, &[]).unwrap_err(),
        PackUpdatePreviewError::MissingProvenance
    );
    let preview = preview_pack_update(Some(&old), &old, &[]).unwrap();
    assert_eq!(preview.unchanged[0].local_state, PackLocalState::Unknown);
    assert!(preview.unchanged[0].preserve);
    let mut target = old.clone();
    target.source.project_id = Some("other".into());
    let mut installed = old.clone();
    installed.source.project_id = Some("original".into());
    assert_eq!(
        preview_pack_update(Some(&installed), &target, &[]).unwrap_err(),
        PackUpdatePreviewError::SourceMismatch
    );
}
#[test]
fn duplicate_paths_and_unobserved_rename_targets_are_not_safe_replacements() {
    let old = pack(vec![file("mods/a.jar", 'a')]);
    let mut target = old.clone();
    target.files[0].path = "mods/b.jar".into();
    let preview = preview_pack_update(Some(&old), &target, &[observed(&old.files[0])]).unwrap();
    assert!(preview.version_changed[0].preserve);
    assert_eq!(
        preview.version_changed[0].local_state,
        PackLocalState::Unknown
    );
    target.files.push(file("Mods/B.jar", 'b'));
    assert_eq!(
        preview_pack_update(Some(&old), &target, &[]).unwrap_err(),
        PackUpdatePreviewError::InvalidManifest
    );
}
#[test]
fn archive_snapshot_hashes_final_client_override_and_detects_same_size_changes() {
    use std::io::Write;
    let path = std::env::temp_dir().join(format!(
        "cobble-pack-preview-{}-{}.mrpack",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let write = |bytes: &[u8]| {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","versionId":"original","name":"Original fixture","files":[],"dependencies":{"minecraft":"1.21.1"}}"#).unwrap();
        for (name, data) in [
            ("overrides/config/example.toml", b"base".as_slice()),
            ("client-overrides/config/example.toml", bytes),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    };
    write(b"first");
    let old = snapshot_mrpack(&path, &[]).unwrap();
    write(b"other");
    let target = snapshot_mrpack(&path, &[]).unwrap();
    let preview = preview_pack_update(Some(&old), &target, &[observed(&old.files[0])]).unwrap();
    assert_eq!(preview.version_changed.len(), 1);
    assert!(preview.version_changed[0].preserve);
    assert_eq!(old.files.len(), 1);
    assert_eq!(old.files[0].size_bytes, 5);
    assert!(snapshot_mrpack(&path, &["unknown.jar".into()]).is_err());
    std::fs::remove_file(path).unwrap();
}

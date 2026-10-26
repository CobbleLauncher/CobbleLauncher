// SPDX-License-Identifier: MIT
use super::*;
fn fixture() -> (tempfile::TempDir, Dir) {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("config")).unwrap();
    fs::write(temp.path().join("config/example.toml"), b"enabled = true\n").unwrap();
    let root = root(temp.path()).unwrap();
    (temp, root)
}
#[test]
fn config_save_rejects_stale_and_retains_original_backup() {
    let (temp, root) = fixture();
    let files = listing(&root).unwrap();
    assert_eq!(files.len(), 1);
    let id = &files[0].id;
    assert!(!id.contains("example"));
    let before = document(&root, id).unwrap();
    assert_eq!(
        replace(&root, id, "changed", "stale").err().unwrap(),
        "CONFIG_CHANGED"
    );
    assert!(backups(&root, id).unwrap().is_empty());
    let after = replace(&root, id, "enabled = false\n", &before.revision).unwrap();
    assert_ne!(after.revision, before.revision);
    assert_eq!(after.backups.len(), 1);
    let saved = backup_dir(&root, id, false).unwrap().unwrap();
    assert_eq!(
        bounded(&saved, Path::new(&after.backups[0].id)).unwrap(),
        before.text.as_bytes()
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("config/example.toml")).unwrap(),
        after.text
    );
    assert_eq!(
        replace(&root, id, "third", &before.revision).err().unwrap(),
        "CONFIG_CHANGED"
    );
}
#[test]
fn config_bounds_identifiers_and_text() {
    let (temp, root) = fixture();
    fs::write(temp.path().join("config/skip.exe"), b"test").unwrap();
    fs::write(temp.path().join("config/binary.cfg"), [255u8]).unwrap();
    let files = listing(&root).unwrap();
    assert_eq!(files.len(), 2);
    let binary = files.iter().find(|f| f.name == "binary.cfg").unwrap();
    assert_eq!(
        document(&root, &binary.id).err().unwrap(),
        "CONFIG_NOT_UTF8"
    );
    assert_eq!(
        selected(&root, "../../other").err().unwrap(),
        "CONFIG_NOT_FOUND"
    );
    let file = files.iter().find(|f| f.name == "example.toml").unwrap();
    let before = document(&root, &file.id).unwrap();
    assert_eq!(
        replace(
            &root,
            &file.id,
            &"a".repeat(LIMIT as usize + 1),
            &before.revision
        )
        .err()
        .unwrap(),
        "CONFIG_TOO_LARGE"
    );
    assert!(backups(&root, &file.id).unwrap().is_empty());
}
#[test]
fn config_backup_restore_keeps_replaced_content() {
    let (_temp, root) = fixture();
    let id = listing(&root).unwrap().remove(0).id;
    let before = document(&root, &id).unwrap();
    let after = replace(&root, &id, "second", &before.revision).unwrap();
    let restored = restore(&root, &id, &after.backups[0].id, &after.revision).unwrap();
    assert_eq!(restored.text, before.text);
    let dir = backup_dir(&root, &id, false).unwrap().unwrap();
    assert!(
        restored
            .backups
            .iter()
            .any(|b| bounded(&dir, Path::new(&b.id)).unwrap() == b"second")
    );
}
#[test]
fn config_backup_preview_and_recovery_metadata_are_bounded_and_scoped() {
    let (temp, root) = fixture();
    let id = listing(&root).unwrap().remove(0).id;
    let before = document(&root, &id).unwrap();
    let after = replace(&root, &id, "second", &before.revision).unwrap();
    let backup_id = after.backups[0].id.clone();
    assert_eq!(
        backup_document(&root, &id, &backup_id).unwrap().text,
        before.text
    );
    assert!(
        matches!(backup_document(&root, "f".repeat(64).as_str(), &backup_id), Err(error) if error == "CONFIG_NOT_FOUND")
    );
    let metadata = recovery_metadata(temp.path()).unwrap();
    assert_eq!(metadata.files.len(), 1);
    assert_eq!(metadata.files[0].id, id);
    assert_eq!(metadata.files[0].name, "example.toml");
    assert_eq!(metadata.files[0].backups[0].id, backup_id);
    assert!(!metadata.truncated);
}
#[test]
fn config_restore_rejects_corrupt_backup_without_overwrite() {
    let (temp, root) = fixture();
    let id = listing(&root).unwrap().remove(0).id;
    let before = document(&root, &id).unwrap();
    let after = replace(&root, &id, "second", &before.revision).unwrap();
    fs::write(
        temp.path()
            .join(".cobble-config-backups")
            .join(&id)
            .join(&after.backups[0].id),
        "corrupted",
    )
    .unwrap();
    assert_eq!(
        restore(&root, &id, &after.backups[0].id, &after.revision)
            .err()
            .unwrap(),
        "CONFIG_BACKUP_CHANGED"
    );
    assert_eq!(document(&root, &id).unwrap().text, "second");
}
#[cfg(windows)]
#[test]
fn config_rejects_directory_junction_escape() {
    let (temp, root) = fixture();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.cfg"), "private").unwrap();
    use std::os::windows::process::CommandExt;
    let link = temp.path().join("config").join("linked");
    let result = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .creation_flags(0x0800_0000)
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(listing(&root).unwrap().len(), 1);
    fs::remove_dir(&link).unwrap();
}

#[test]
fn config_save_preserves_crlf_and_utf8_bom() {
    let (temp, root) = fixture();
    fs::write(
        temp.path().join("config/example.toml"),
        "\u{feff}one = true\r\ntwo = false\r\n",
    )
    .unwrap();
    let id = listing(&root).unwrap().remove(0).id;
    let before = document(&root, &id).unwrap();
    let after = save(&root, &id, "one = false\ntwo = false\n", &before.revision).unwrap();
    assert_eq!(after.text, "\u{feff}one = false\r\ntwo = false\r\n");
    let restored = restore(&root, &id, &after.backups[0].id, &after.revision).unwrap();
    assert_eq!(restored.text, before.text);
}

#[test]
fn report_snapshot_is_revision_bound_immutable_and_read_only() {
    let (temp, root) = fixture();
    let selected = listing(&root).unwrap().remove(0);
    let before = document(&root, &selected.id).unwrap();
    let (bytes, extension) = report_snapshot(temp.path(), &selected.id, &before.revision).unwrap();
    assert_eq!(extension, "toml");
    assert_eq!(bytes, before.text.as_bytes());
    assert!(!temp.path().join(".cobble-config-backups").exists());
    assert_eq!(
        fs::read(temp.path().join("config/example.toml")).unwrap(),
        bytes
    );
    assert_eq!(
        report_snapshot(temp.path(), "../../outside", &before.revision).unwrap_err(),
        "CONFIG_NOT_FOUND"
    );
    fs::write(
        temp.path().join("config/example.toml"),
        b"external = true\n",
    )
    .unwrap();
    assert_eq!(
        report_snapshot(temp.path(), &selected.id, &before.revision).unwrap_err(),
        "CONFIG_CHANGED"
    );
    assert_eq!(bytes, b"enabled = true\n");
}

#[test]
fn shared_config_adoption_backs_up_existing_and_removes_only_unchanged_new_file() {
    let (temp, root) = fixture();
    let path = "config/example.toml";
    let (id, original, revision) = inspect_shared_config(temp.path(), path).unwrap().unwrap();
    let (_, backup, changed) =
        adopt_shared_config(temp.path(), path, "enabled = false\n", Some(&revision)).unwrap();
    let backup = backup.unwrap();
    assert_eq!(backup_document(&root, &id, &backup).unwrap().text, original);
    assert_eq!(
        inspect_shared_config(temp.path(), path).unwrap().unwrap().2,
        changed
    );
    assert_eq!(
        adopt_shared_config(temp.path(), path, "stale", Some(&revision)).unwrap_err(),
        "CHANGE_SHARE_CONFIG_CHANGED"
    );
    let fresh = "config/new/sub.json";
    let (_, none, new_revision) = adopt_shared_config(temp.path(), fresh, "{}", None).unwrap();
    assert!(none.is_none());
    fs::write(temp.path().join(fresh), b"later edit").unwrap();
    assert_eq!(
        remove_shared_created_config(temp.path(), fresh, &new_revision).unwrap_err(),
        "CHANGE_SHARE_CONFIG_CHANGED"
    );
    fs::write(temp.path().join(fresh), b"{}").unwrap();
    remove_shared_created_config(temp.path(), fresh, &new_revision).unwrap();
    assert!(!temp.path().join(fresh).exists());
}

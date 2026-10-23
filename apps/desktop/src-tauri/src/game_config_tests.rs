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

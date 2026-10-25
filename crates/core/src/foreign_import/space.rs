// SPDX-License-Identifier: MIT
use super::*;

/// Measures only the same allowlisted content the importer copies. Existing
/// source bytes already occupy their volume and aren't additional free demand.
pub fn launcher_import_space_demand(
    candidate: &LauncherImportCandidate,
    content_id: &str,
    target: &Path,
    include_saves: bool,
    control: &crate::OperationControl,
) -> Result<(PathBuf, u64), Error> {
    control.check_cancelled()?;
    if let Some(reason) = &candidate.unsupported_reason {
        return Err(reason.clone().into());
    }
    let mut checked = candidate.clone();
    runtime_supported(&mut checked)?;
    let source = candidate
        .content_roots
        .iter()
        .find(|root| root.id == content_id)
        .ok_or("请选择要迁移的内容目录")?
        .directory
        .clone();
    if target.try_exists()? {
        real_directory(target)?;
        let state = read_state(target)?;
        if Some(&state.minecraft_version) != candidate.minecraft_version.as_ref()
            || state.loader_kind != candidate.loader_kind
            || state.loader_version != candidate.loader_version
            || state.source != source
            || state.content_id != content_id
            || state.include_saves != include_saves
        {
            return Err("目标属于另一个导入任务，未修改任何内容".into());
        }
        // Publication is a rename, and already published content isn't copied again.
        if state.staging_complete || state.content_complete {
            return Ok((source, 0));
        }
    }
    let source = real_directory(&source)?;
    let mut bytes = 0u64;
    for name in selected_entries(&source, include_saves)? {
        bytes = bytes
            .checked_add(entry_bytes(&source.join(name), false, 0, control)?)
            .ok_or("SPACE_OVERFLOW")?;
    }
    Ok((source, bytes))
}

fn entry_bytes(
    source: &Path,
    world_root: bool,
    depth: usize,
    control: &crate::OperationControl,
) -> Result<u64, Error> {
    control.check_cancelled()?;
    if depth > 128 {
        return Err("导入内容目录超过 128 层".into());
    }
    let metadata = fs::symlink_metadata(source)?;
    if linked(&metadata) {
        return Err("导入内容不能含链接或重解析点".into());
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Err("导入内容含不支持的特殊文件".into());
    }
    let mut bytes = 0u64;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().ok_or("导入文件名必须是 UTF-8")?;
        leaf(name)?;
        if world_root && name == "session.lock" {
            continue;
        }
        bytes = bytes
            .checked_add(entry_bytes(
                &entry.path(),
                depth == 0 && source.file_name().is_some_and(|name| name == "saves"),
                depth + 1,
                control,
            )?)
            .ok_or("SPACE_OVERFLOW")?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_space_excludes_private_runtime_and_unselected_worlds_and_observes_cancel() {
        let workspace = std::env::temp_dir().join(format!(
            "cobble-import-space-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = workspace.join("source");
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::create_dir_all(root.join("saves/world")).unwrap();
        fs::write(root.join("mods/a.jar"), [0; 3]).unwrap();
        fs::write(root.join("saves/world/level.dat"), [0; 5]).unwrap();
        fs::write(root.join("saves/world/session.lock"), [0; 8]).unwrap();
        fs::write(root.join("accounts.json"), [0; 20]).unwrap();
        let candidate = LauncherImportCandidate {
            runtime_source: None,
            id: "fixture".into(),
            name: "fixture".into(),
            minecraft_version: Some("1.21.1".into()),
            loader_kind: None,
            loader_version: None,
            unsupported_reason: None,
            warnings: vec![],
            content_roots: vec![LauncherContentRoot {
                id: "selected".into(),
                label: "fixture".into(),
                has_saves: true,
                directory: root.clone(),
            }],
        };
        let control = crate::OperationControl::new(Arc::new(|_| {}));
        let target = workspace.join("target");
        assert_eq!(
            launcher_import_space_demand(&candidate, "selected", &target, false, &control)
                .unwrap()
                .1,
            3
        );
        assert_eq!(
            launcher_import_space_demand(&candidate, "selected", &target, true, &control)
                .unwrap()
                .1,
            8
        );
        let state = ImportState {
            runtime_source: None,
            released_roots: vec![],
            minecraft_version: "1.21.1".into(),
            loader_kind: None,
            loader_version: None,
            source: root.clone(),
            content_id: "selected".into(),
            include_saves: true,
            entries: vec![],
            staging_complete: true,
            content_complete: true,
            complete: false,
        };
        initialize_target(&target, &state).unwrap();
        assert_eq!(
            launcher_import_space_demand(&candidate, "selected", &target, true, &control)
                .unwrap()
                .1,
            0
        );
        assert!(
            launcher_import_space_demand(&candidate, "selected", &target, false, &control).is_err()
        );
        control.cancel();
        assert!(
            launcher_import_space_demand(&candidate, "selected", &target, true, &control).is_err()
        );
        fs::remove_dir_all(workspace).unwrap();
    }
}

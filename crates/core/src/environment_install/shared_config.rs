// SPDX-License-Identifier: MIT
use super::Error;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    path::Path,
};
const MARKER: &str = ".cobble-environment-config.json";
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedConfig {
    pub path: String,
    pub text: String,
    pub sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    schema_version: u32,
    identity: String,
    complete: bool,
}
pub fn validate_shared_configs(configs: &[SharedConfig]) -> Result<(), Error> {
    if configs.len() > 64 {
        return Err("ENVIRONMENT_CONFIG_LIMIT".into());
    }
    let mut names = BTreeSet::<String>::new();
    let mut total = 0usize;
    for config in configs {
        let safe = cobble_store::SafeRelativePath::parse(&config.path)?;
        let normalized = safe.as_str().to_ascii_lowercase();
        if !safe.as_str().starts_with("config/")
            || safe.as_str().len() > 512
            || config.text.len() > 65536
            || names.iter().any(|existing| {
                normalized
                    .strip_prefix(existing.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
                    || existing
                        .strip_prefix(normalized.as_str())
                        .is_some_and(|tail| tail.starts_with('/'))
            })
            || !names.insert(normalized)
            || config.sha256.len() != 64
            || !format!("{:x}", Sha256::digest(config.text.as_bytes()))
                .eq_ignore_ascii_case(&config.sha256)
        {
            return Err("ENVIRONMENT_CONFIG_INVALID".into());
        }
        total = total
            .checked_add(config.text.len())
            .ok_or("ENVIRONMENT_CONFIG_LIMIT")?;
    }
    if total > 512 * 1024 {
        return Err("ENVIRONMENT_CONFIG_LIMIT".into());
    }
    Ok(())
}
fn read(root: &Dir) -> Result<Option<Receipt>, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = match root.open_with(MARKER, &options) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if !file.metadata()?.is_file() || file.metadata()?.len() > 4096 {
        return Err("ENVIRONMENT_CONFIG_RECEIPT_INVALID".into());
    }
    let receipt: Receipt = serde_json::from_reader(file.take(4097))?;
    if receipt.schema_version != 1 || receipt.identity.len() != 64 {
        return Err("ENVIRONMENT_CONFIG_RECEIPT_INVALID".into());
    }
    Ok(Some(receipt))
}
fn write(root: &Dir, receipt: &Receipt) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(format!("{MARKER}.tmp"), &options)?;
    serde_json::to_writer(&mut file, receipt)?;
    file.sync_all()?;
    drop(file);
    root.rename(format!("{MARKER}.tmp"), root, MARKER)?;
    Ok(())
}
pub(super) fn ensure_complete(directory: &Path) -> Result<(), Error> {
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    if read(&root)?.is_some_and(|r| !r.complete) {
        return Err("ENVIRONMENT_CONFIG_INCOMPLETE".into());
    }
    Ok(())
}
pub fn install_shared_configs_controlled(
    configs: &[SharedConfig],
    directory: &Path,
    control: crate::OperationControl,
) -> Result<(), Error> {
    validate_shared_configs(configs)?;
    if configs.is_empty() {
        return Ok(());
    }
    control.check_cancelled()?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let identity = format!("{:x}", Sha256::digest(serde_json::to_vec(configs)?));
    let previous = read(&root)?;
    if previous.as_ref().is_some_and(|r| r.identity != identity) {
        return Err("ENVIRONMENT_CONFIG_PLAN_CHANGED".into());
    }
    // Check every destination before writing the pending record. Existing
    // different configuration belongs to the player and is never replaced.
    for (index, config) in configs.iter().enumerate() {
        if let Some(mut file) = crate::readiness::open_scoped(&root, &config.path)? {
            let mut existing = Vec::new();
            std::io::Read::by_ref(&mut file)
                .take(65537)
                .read_to_end(&mut existing)?;
            if existing != config.text.as_bytes() {
                return Err("ENVIRONMENT_CONFIG_CONFLICT".into());
            }
        }
        let parent = config
            .path
            .rsplit_once('/')
            .ok_or("ENVIRONMENT_CONFIG_INVALID")?
            .0;
        let temporary = format!("{parent}/.cobble-shared-{}-{index}.tmp", &identity[..16]);
        if crate::readiness::open_scoped(&root, &temporary)?.is_some()
            && previous.as_ref().is_none_or(|r| r.complete)
        {
            return Err("ENVIRONMENT_CONFIG_CONFLICT".into());
        }
    }
    write(
        &root,
        &Receipt {
            schema_version: 1,
            identity: identity.clone(),
            complete: false,
        },
    )?;
    for (index, config) in configs.iter().enumerate() {
        control.check_cancelled()?;
        let mut parent = root.try_clone()?;
        let components: Vec<_> = config.path.split('/').collect();
        for part in &components[..components.len() - 1] {
            parent = match parent.open_dir_nofollow(part) {
                Ok(dir) => dir,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    parent.create_dir(part)?;
                    parent.open_dir_nofollow(part)?
                }
                Err(e) => return Err(e.into()),
            };
        }
        let name = components[components.len() - 1];
        if crate::readiness::open_scoped(&root, &config.path)?.is_none() {
            let temporary = format!(".cobble-shared-{}-{index}.tmp", &identity[..16]);
            let mut options = OpenOptions::new();
            options
                .write(true)
                .create(true)
                .truncate(true)
                .follow(FollowSymlinks::No);
            let mut file = parent.open_with(&temporary, &options)?;
            file.write_all(config.text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            control.check_cancelled()?;
            parent.rename(&temporary, &parent, name)?;
        }
        (control.progress())(crate::InstallProgress {
            detail: None,
            phase: "shared-config".into(),
            completed: index + 1,
            total: configs.len(),
            reused: 0,
        });
    }
    write(
        &root,
        &Receipt {
            schema_version: 1,
            identity,
            complete: true,
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(path: &str, text: &str) -> SharedConfig {
        SharedConfig {
            path: path.into(),
            text: text.into(),
            sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        }
    }
    #[test]
    fn shared_config_refuses_personal_paths_duplicates_and_digest_drift() {
        assert!(validate_shared_configs(&[config("options.txt", "private")]).is_err());
        assert!(
            validate_shared_configs(&[
                config("config/a.json", "{}"),
                config("config/A.json", "{}")
            ])
            .is_err()
        );
        assert!(
            validate_shared_configs(&[
                config("config/folder", "x"),
                config("config/folder/file.json", "{}")
            ])
            .is_err()
        );
        let mut changed = config("config/a.json", "{}");
        changed.text = "changed".into();
        assert!(validate_shared_configs(&[changed]).is_err());
    }
    #[test]
    fn shared_config_partial_cancel_blocks_launch_and_same_plan_resumes_without_changing_existing_text()
     {
        let path = std::env::temp_dir().join(format!(
            "cobble-shared-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let configs = [
            config("config/a.json", "{\"a\":1}"),
            config("config/nested/b.json", "{\"b\":2}"),
        ];
        let holder = std::sync::Arc::new(std::sync::Mutex::new(None::<crate::OperationControl>));
        let captured = holder.clone();
        let control = crate::OperationControl::new(std::sync::Arc::new(move |p| {
            if p.completed == 1 {
                captured.lock().unwrap().as_ref().unwrap().cancel();
            }
        }));
        *holder.lock().unwrap() = Some(control.clone());
        assert!(install_shared_configs_controlled(&configs, &path, control).is_err());
        assert!(ensure_complete(&path).is_err());
        assert_eq!(
            std::fs::read(path.join("config/a.json")).unwrap(),
            configs[0].text.as_bytes()
        );
        install_shared_configs_controlled(
            &configs,
            &path,
            crate::OperationControl::new(std::sync::Arc::new(|_| {})),
        )
        .unwrap();
        ensure_complete(&path).unwrap();
        std::fs::write(path.join("config/a.json"), b"player changed").unwrap();
        assert!(
            install_shared_configs_controlled(
                &configs,
                &path,
                crate::OperationControl::new(std::sync::Arc::new(|_| {}))
            )
            .is_err()
        );
        assert_eq!(
            std::fs::read(path.join("config/a.json")).unwrap(),
            b"player changed"
        );
        std::fs::remove_dir_all(path).unwrap();
    }
}

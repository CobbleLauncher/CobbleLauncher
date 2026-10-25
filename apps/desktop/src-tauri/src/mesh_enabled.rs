// SPDX-License-Identifier: MIT
//! Whole-Mesh opt-out. All network startup checks share the broker lock.
use super::*;
use std::io::{Read, Write};
fn path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("MESH_STORAGE_UNAVAILABLE")?
        .join("CobbleLauncher/mesh-enabled.json"))
}
pub(super) fn current() -> bool {
    path().map(|path| read_at(&path)).unwrap_or(false)
}
fn legacy_identity_exists() -> bool {
    dirs::data_local_dir()
        .map(|root| legacy_identity_at(&root.join("CobbleLauncher/mesh-identity/identity.json")))
        .unwrap_or(false)
}
fn legacy_identity_at(path: &std::path::Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > 16384
    {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}
fn read_at(path: &std::path::Path) -> bool {
    read_with_default(path, legacy_identity_exists)
}
fn read_with_default(path: &std::path::Path, legacy_enabled: impl FnOnce() -> bool) -> bool {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return legacy_enabled(),
        Err(_) => return false,
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 128 {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    let mut bytes = Vec::new();
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    if file.take(129).read_to_end(&mut bytes).is_err() {
        return false;
    }
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .filter(|v| v["schema_version"] == 1)
        .and_then(|v| v["enabled"].as_bool())
        .unwrap_or(false)
}
pub(super) fn is_enabled(broker: &mut Broker) -> bool {
    *broker
        .enabled
        .get_or_insert_with(|| path().map(|p| read_at(&p)).unwrap_or(false))
}
pub(super) fn require(broker: &mut Broker) -> Result<(), String> {
    if is_enabled(broker) && current() {
        Ok(())
    } else {
        Err("MESH_DISABLED".into())
    }
}
pub(super) fn save(value: bool) -> Result<(), String> {
    save_at(&path()?, value)
}
fn save_at(path: &std::path::Path, value: bool) -> Result<(), String> {
    let parent = path.parent().ok_or("MESH_STORAGE_UNAVAILABLE")?;
    std::fs::create_dir_all(parent).map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    let metadata = std::fs::symlink_metadata(parent).map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("MESH_STORAGE_UNAVAILABLE".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("MESH_STORAGE_UNAVAILABLE".into());
        }
    }
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    file.write_all(
        &serde_json::to_vec(&json!({"schema_version":1,"enabled":value}))
            .map_err(|_| "MESH_STORAGE_UNAVAILABLE")?,
    )
    .map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    file.persist(path).map_err(|_| "MESH_STORAGE_UNAVAILABLE")?;
    Ok(())
}
pub(super) fn stop_all(broker: &mut Broker) {
    groups::clear();
    for slot in [
        &mut broker.client,
        &mut broker.discovery_client,
        &mut broker.social_client,
    ] {
        if let Some(client) = slot.take() {
            client.shutdown();
        }
    }
    broker.binding.take();
    broker.discovery = Default::default();
    broker.discovery_checked_at = None;
    broker.temporary_identity.take();
    broker.social_generation = broker.social_generation.wrapping_add(1);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preference_roundtrip_and_corrupt_fail_closed_without_user_data() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("mesh-enabled.json");
        assert!(!read_with_default(&path, || false));
        assert!(read_with_default(&path, || true));
        save_at(&path, false).unwrap();
        assert!(!read_with_default(&path, || true));
        save_at(&path, true).unwrap();
        assert!(read_with_default(&path, || false));
        std::fs::write(&path, b"broken").unwrap();
        assert!(!read_with_default(&path, || true));
    }
    #[test]
    fn offline_gate_blocks_prepare_and_clears_cached_state() {
        let mut broker = Broker {
            enabled: Some(false),
            ..Default::default()
        };
        assert_eq!(broker.prepare().unwrap_err(), "MESH_DISABLED");
        assert!(broker.client.is_none());
        broker.discovery.state = "published".into();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().to_owned();
        broker.temporary_identity = Some(temp);
        stop_all(&mut broker);
        assert!(!path.exists());
        assert!(broker.discovery.nodes.is_empty());
        assert!(broker.discovery.state.is_empty());
        assert!(require(&mut broker).is_err());
    }
    #[test]
    fn legacy_identity_requires_an_actual_nonempty_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("identity.json");
        assert!(!legacy_identity_at(&path));
        std::fs::create_dir(&path).unwrap();
        assert!(!legacy_identity_at(&path));
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"").unwrap();
        assert!(!legacy_identity_at(&path));
        std::fs::write(&path, b"saved identity fixture").unwrap();
        assert!(legacy_identity_at(&path));
    }
}

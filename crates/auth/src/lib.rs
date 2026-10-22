// SPDX-License-Identifier: MIT
//! Local player profiles, Microsoft sign-in and OS-backed credentials.
//!
//! Offline profiles are local identities, not verified Microsoft accounts.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

mod microsoft;
mod protocol;
mod vault;
pub use microsoft::{
    DeviceLogin, LaunchIdentity, LoginPoll, begin_microsoft_login, cancel_microsoft_login,
    launch_identity, microsoft_login_url, poll_microsoft_login,
};
pub use protocol::AuthError;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Public desktop application ID; no client secret is shipped.
pub const MICROSOFT_CLIENT_ID: &str = "237f1625-ed8a-421c-ab8e-20448c0eb448";

/// A local identity for offline play. It does not authenticate to online services.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineProfile {
    pub name: String,
    /// Standard Java offline UUID, formatted as 32 lowercase hexadecimal digits.
    pub uuid: String,
}

impl OfflineProfile {
    /// Create an offline identity without changing the saved active profile.
    pub fn new(name: &str) -> Result<Self, BoxError> {
        if !(3..=16).contains(&name.len())
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Offline name must be 3–16 ASCII letters, digits, or underscores",
            )
            .into());
        }

        // Java's nameUUIDFromBytes uses raw MD5 followed by RFC UUID version/variant
        // tagging. A UUID namespace must not be prepended to OfflinePlayer:<name>.
        // MD5 is required for format compatibility, not used for authentication.
        let mut bytes = md5::compute(format!("OfflinePlayer:{name}").as_bytes()).0;
        bytes[6] = (bytes[6] & 0x0f) | 0x30;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Ok(Self {
            name: name.to_owned(),
            uuid: uuid::Uuid::from_bytes(bytes).simple().to_string(),
        })
    }
}

/// Saved player identity. Offline names and UUIDs are immutable after creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerProfile {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileLibrary {
    pub profiles: Vec<PlayerProfile>,
    pub active_id: Option<String>,
}

// Serialize native command access, including the first legacy migration.
static PROFILE_ACCESS: Mutex<()> = Mutex::new(());

fn config_directory() -> Result<PathBuf, BoxError> {
    let config = dirs::config_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User configuration directory unavailable",
        )
    })?;
    Ok(config.join("CobbleLauncher"))
}

fn with_library<T>(
    action: impl FnOnce(&Path, ProfileLibrary) -> Result<T, BoxError>,
) -> Result<T, BoxError> {
    let _guard = PROFILE_ACCESS
        .lock()
        .map_err(|_| io::Error::other("Profile storage lock unavailable"))?;
    let directory = config_directory()?;
    let library = read_library_at(&directory)?;
    action(&directory, library)
}

pub fn list_player_profiles() -> Result<ProfileLibrary, BoxError> {
    with_library(|_, library| Ok(library))
}

/// Add and select an offline identity, or select the existing exact-name match.
pub fn add_offline_profile(name: &str, note: Option<&str>) -> Result<ProfileLibrary, BoxError> {
    with_library(|directory, mut library| {
        add_to_library(&mut library, name, note)?;
        save_library_at(directory, &library)?;
        Ok(library)
    })
}

pub fn select_player_profile(id: &str) -> Result<ProfileLibrary, BoxError> {
    with_library(|directory, mut library| {
        find_profile(&library, id)?;
        library.active_id = Some(id.to_owned());
        save_library_at(directory, &library)?;
        Ok(library)
    })
}

pub fn update_profile_note(id: &str, note: Option<&str>) -> Result<ProfileLibrary, BoxError> {
    with_library(|directory, mut library| {
        update_note_in_library(&mut library, id, note)?;
        save_library_at(directory, &library)?;
        Ok(library)
    })
}

pub fn remove_player_profile(id: &str) -> Result<ProfileLibrary, BoxError> {
    with_library(|directory, mut library| {
        if find_profile(&library, id)?.kind == "microsoft" {
            microsoft::revoke(id)?;
        }
        remove_from_library(&mut library, id)?;
        save_library_at(directory, &library)?;
        Ok(library)
    })
}

/// Capture the effective saved identity without performing authentication or I/O
/// to a remote service. Explicit missing bindings never fall back to another.
pub fn get_player_profile(id: Option<&str>) -> Result<PlayerProfile, BoxError> {
    with_library(|_, library| {
        let id = id.or(library.active_id.as_deref()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Choose a player profile before launching",
            )
        })?;
        Ok(find_profile(&library, id)?.clone())
    })
}

/// Return an identity snapshot. An explicit missing ID never falls back to active.
pub fn resolve_player_profile(id: Option<&str>) -> Result<OfflineProfile, BoxError> {
    with_library(|_, library| resolve_from_library(&library, id))
}

/// Compatibility API: read the current selection without discarding other profiles.
pub fn read_offline_profile() -> Result<Option<OfflineProfile>, BoxError> {
    with_library(|_, library| match library.active_id.as_deref() {
        Some(id) => resolve_from_library(&library, Some(id)).map(Some),
        None => Ok(None),
    })
}

/// Compatibility API: add/select an identity without replacing the library.
pub fn save_offline_profile(name: &str) -> Result<OfflineProfile, BoxError> {
    let library = add_offline_profile(name, None)?;
    resolve_from_library(&library, None)
}

/// Compatibility API: remove only the active identity, preserving other entries.
pub fn remove_offline_profile() -> Result<(), BoxError> {
    with_library(|directory, mut library| {
        if let Some(id) = library.active_id.clone() {
            resolve_from_library(&library, Some(&id))?;
            remove_from_library(&mut library, &id)?;
            save_library_at(directory, &library)?;
        }
        Ok(())
    })
}

fn find_profile<'a>(library: &'a ProfileLibrary, id: &str) -> Result<&'a PlayerProfile, BoxError> {
    library
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Player profile not found; choose a player profile",
            )
            .into()
        })
}

fn resolve_from_library(
    library: &ProfileLibrary,
    id: Option<&str>,
) -> Result<OfflineProfile, BoxError> {
    let id = id.or(library.active_id.as_deref()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Choose a player profile before launching",
        )
    })?;
    let profile = find_profile(library, id)?;
    if profile.kind != "offline" {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Unsupported player profile kind",
        )
        .into());
    }
    Ok(OfflineProfile {
        name: profile.name.clone(),
        uuid: profile.uuid.clone(),
    })
}

fn add_to_library(
    library: &mut ProfileLibrary,
    name: &str,
    note: Option<&str>,
) -> Result<(), BoxError> {
    let offline = OfflineProfile::new(name)?;
    if let Some(existing) = library
        .profiles
        .iter()
        .find(|profile| profile.kind == "offline" && profile.name == name)
    {
        library.active_id = Some(existing.id.clone());
        return Ok(());
    }
    let profile = player_from_offline(offline, note);
    library.active_id = Some(profile.id.clone());
    library.profiles.push(profile);
    Ok(())
}

fn player_from_offline(offline: OfflineProfile, note: Option<&str>) -> PlayerProfile {
    PlayerProfile {
        id: format!("offline-{}", offline.uuid),
        kind: "offline".to_owned(),
        name: offline.name,
        uuid: offline.uuid,
        note: note.map(str::to_owned),
    }
}

fn remove_from_library(library: &mut ProfileLibrary, id: &str) -> Result<(), BoxError> {
    find_profile(library, id)?;
    library.profiles.retain(|profile| profile.id != id);
    if library.active_id.as_deref() == Some(id) {
        library.active_id = None;
    }
    Ok(())
}

fn update_note_in_library(
    library: &mut ProfileLibrary,
    id: &str,
    note: Option<&str>,
) -> Result<(), BoxError> {
    let profile = library
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Player profile not found; choose a player profile",
            )
        })?;
    profile.note = note.map(str::to_owned);
    Ok(())
}

fn validate_library(library: &ProfileLibrary) -> Result<(), BoxError> {
    let mut ids = std::collections::HashSet::new();
    let mut names = std::collections::HashSet::new();
    for profile in &library.profiles {
        match profile.kind.as_str() {
            "offline" => {
                OfflineProfile::new(&profile.name)?;
            }
            "microsoft"
                if !profile.name.is_empty()
                    && profile.name.len() <= 16
                    && profile
                        .name
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    && profile.id == format!("microsoft-{}", profile.uuid) => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid player profile kind or identity",
                )
                .into());
            }
        }
        uuid::Uuid::parse_str(&profile.uuid)?;
        if profile.id.is_empty()
            || !ids.insert(&profile.id)
            || (profile.kind == "offline" && !names.insert(&profile.name))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid or duplicate player profile",
            )
            .into());
        }
    }
    if let Some(id) = &library.active_id {
        find_profile(library, id)?;
    }
    Ok(())
}

fn read_library_at(directory: &Path) -> Result<ProfileLibrary, BoxError> {
    let path = directory.join("player-profiles.json");
    match fs::read(&path) {
        Ok(bytes) => {
            let library: ProfileLibrary = serde_json::from_slice(&bytes)?;
            validate_library(&library)?;
            return Ok(library);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let bytes = match fs::read(directory.join("offline-profile.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ProfileLibrary::default());
        }
        Err(error) => return Err(error.into()),
    };
    let offline: OfflineProfile = serde_json::from_slice(&bytes)?;
    let profile = player_from_offline(offline, None);
    let library = ProfileLibrary {
        active_id: Some(profile.id.clone()),
        profiles: vec![profile],
    };
    // The old file remains a recovery copy. Once saved, even an empty library
    // takes precedence, so removed profiles cannot be resurrected by migration.
    save_library_at(directory, &library)?;
    Ok(library)
}

fn save_library_at(directory: &Path, library: &ProfileLibrary) -> Result<(), BoxError> {
    validate_library(library)?;
    fs::create_dir_all(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer_pretty(&mut temporary, library)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(directory.join("player-profiles.json"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microsoft_profiles_with_transferred_names_keep_distinct_uuid_identities() {
        let directory = tempfile::tempdir().unwrap();
        let library = ProfileLibrary {
            profiles: [
                "0123456789abcdef0123456789abcdef",
                "fedcba9876543210fedcba9876543210",
            ]
            .into_iter()
            .map(|uuid| PlayerProfile {
                id: format!("microsoft-{uuid}"),
                kind: "microsoft".into(),
                name: "TransferredName".into(),
                uuid: uuid.into(),
                note: None,
            })
            .collect(),
            active_id: None,
        };
        save_library_at(directory.path(), &library).unwrap();
        assert_eq!(read_library_at(directory.path()).unwrap(), library);
        assert_ne!(library.profiles[0].id, library.profiles[1].id);
    }

    #[test]
    fn standard_offline_uuid_is_case_sensitive() {
        let notch = OfflineProfile::new("Notch").unwrap();
        assert_eq!(notch.uuid, "b50ad385829d3141a2167e7d7539ba7f");
        assert_ne!(notch.uuid, OfflineProfile::new("notch").unwrap().uuid);
        assert_eq!(notch.name, "Notch");
    }

    #[test]
    fn names_have_explicit_ascii_bounds() {
        for name in [
            "",
            "ab",
            "abcdefghijklmnopq",
            "a b",
            "abc\n",
            "玩家名",
            "../x",
        ] {
            assert!(OfflineProfile::new(name).is_err(), "accepted {name:?}");
        }
        for name in ["abc", "Player_123", "abcdefghijklmnop"] {
            assert!(OfflineProfile::new(name).is_ok(), "rejected {name:?}");
        }
    }

    #[test]
    fn multiple_profiles_reload_select_and_remove_without_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let mut library = read_library_at(path).unwrap();
        assert_eq!(library, ProfileLibrary::default());
        add_to_library(&mut library, "Dylan", Some("Survival")).unwrap();
        let first_id = library.active_id.clone().unwrap();
        add_to_library(&mut library, "Builder", None).unwrap();
        let second_id = library.active_id.clone().unwrap();
        save_library_at(path, &library).unwrap();
        let mut library = read_library_at(path).unwrap();
        assert_eq!(library.profiles.len(), 2);
        assert_eq!(
            resolve_from_library(&library, None).unwrap().name,
            "Builder"
        );
        assert_eq!(
            resolve_from_library(&library, Some(&first_id))
                .unwrap()
                .name,
            "Dylan"
        );
        assert!(resolve_from_library(&library, Some("missing")).is_err());
        // Selecting an exact duplicate does not alter the identity or its note.
        add_to_library(&mut library, "Dylan", Some("Replacement")).unwrap();
        assert_eq!(library.profiles.len(), 2);
        assert_eq!(library.active_id.as_deref(), Some(first_id.as_str()));
        assert_eq!(library.profiles[0].note.as_deref(), Some("Survival"));
        let snapshot = resolve_from_library(&library, None).unwrap();
        library.active_id = Some(second_id.clone());
        assert_eq!(snapshot.name, "Dylan");
        remove_from_library(&mut library, &second_id).unwrap();
        assert_eq!(library.active_id, None);
        assert!(resolve_from_library(&library, None).is_err());
        assert!(resolve_from_library(&library, Some(&second_id)).is_err());
        assert_eq!(library.profiles[0].id, first_id);
        save_library_at(path, &library).unwrap();
        assert_eq!(read_library_at(path).unwrap(), library);
    }

    #[test]
    fn migration_preserves_legacy_identity_and_never_resurrects_removed_profiles() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        // Deliberately use a valid UUID that differs from today's derived UUID.
        let legacy = OfflineProfile {
            name: "Legacy".to_owned(),
            uuid: "0123456789abcdef0123456789abcdef".to_owned(),
        };
        let legacy_bytes = serde_json::to_vec(&legacy).unwrap();
        fs::write(path.join("offline-profile.json"), &legacy_bytes).unwrap();
        let mut library = read_library_at(path).unwrap();
        assert_eq!(resolve_from_library(&library, None).unwrap(), legacy);
        assert_eq!(library.profiles.len(), 1);
        assert_eq!(
            fs::read(path.join("offline-profile.json")).unwrap(),
            legacy_bytes
        );
        let id = library.active_id.clone().unwrap();
        remove_from_library(&mut library, &id).unwrap();
        save_library_at(path, &library).unwrap();
        assert_eq!(read_library_at(path).unwrap(), ProfileLibrary::default());
    }

    #[test]
    fn case_variants_are_distinct_and_invalid_names_do_not_mutate_library() {
        let mut library = ProfileLibrary::default();
        add_to_library(&mut library, "Dylan", None).unwrap();
        add_to_library(&mut library, "dylan", None).unwrap();
        assert_eq!(library.profiles.len(), 2);
        assert_ne!(library.profiles[0].id, library.profiles[1].id);
        let before = library.clone();
        assert!(add_to_library(&mut library, "bad name", None).is_err());
        assert_eq!(library, before);
        assert!(remove_from_library(&mut library, "missing").is_err());
        assert_eq!(library, before);
    }

    #[test]
    fn migration_write_failure_retains_legacy_and_corrupt_new_storage_is_not_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let legacy_bytes = serde_json::to_vec(&OfflineProfile::new("Legacy").unwrap()).unwrap();
        fs::write(path.join("offline-profile.json"), &legacy_bytes).unwrap();
        // A directory at the destination forces a storage error on every OS.
        fs::create_dir(path.join("player-profiles.json")).unwrap();
        let mut library = ProfileLibrary::default();
        add_to_library(&mut library, "Legacy", None).unwrap();
        assert!(save_library_at(path, &library).is_err());
        assert!(read_library_at(path).is_err());
        assert_eq!(
            fs::read(path.join("offline-profile.json")).unwrap(),
            legacy_bytes
        );
        fs::remove_dir(path.join("player-profiles.json")).unwrap();
        fs::write(path.join("player-profiles.json"), b"invalid json").unwrap();
        assert!(read_library_at(path).is_err());
        assert_eq!(
            fs::read(path.join("player-profiles.json")).unwrap(),
            b"invalid json"
        );
    }

    #[test]
    fn notes_reload_and_failed_save_keeps_existing_library() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let mut library = ProfileLibrary::default();
        add_to_library(&mut library, "Dylan", None).unwrap();
        let id = library.active_id.clone().unwrap();
        let identity = resolve_from_library(&library, None).unwrap();
        update_note_in_library(&mut library, &id, Some("Creative worlds")).unwrap();
        save_library_at(path, &library).unwrap();
        assert_eq!(read_library_at(path).unwrap(), library);
        assert_eq!(resolve_from_library(&library, None).unwrap(), identity);
        assert!(update_note_in_library(&mut library, "missing", None).is_err());
        assert_eq!(read_library_at(path).unwrap(), library);
        update_note_in_library(&mut library, &id, None).unwrap();
        save_library_at(path, &library).unwrap();
        assert_eq!(read_library_at(path).unwrap().profiles[0].note, None);
        let mut invalid = library.clone();
        invalid.profiles[0].name = "invalid name".to_owned();
        assert!(save_library_at(path, &invalid).is_err());
        assert_eq!(read_library_at(path).unwrap(), library);
    }
}

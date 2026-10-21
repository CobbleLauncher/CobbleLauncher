// SPDX-License-Identifier: MIT
//! Local offline profiles and Microsoft application configuration.
//!
//! Offline profiles are local identities, not verified Microsoft accounts.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

fn profile_path() -> Result<PathBuf, BoxError> {
    let config = dirs::config_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "User configuration directory unavailable",
        )
    })?;
    Ok(config.join("CobbleLauncher").join("offline-profile.json"))
}

/// Read the single saved active offline profile. Missing storage means signed out.
pub fn read_offline_profile() -> Result<Option<OfflineProfile>, BoxError> {
    read_profile_at(&profile_path()?)
}

/// Replace the saved active offline profile. No tokens or credentials are stored.
pub fn save_offline_profile(name: &str) -> Result<OfflineProfile, BoxError> {
    let profile = OfflineProfile::new(name)?;
    save_profile_at(&profile_path()?, &profile)?;
    Ok(profile)
}

/// Remove the saved offline profile without changing game files or worlds.
pub fn remove_offline_profile() -> Result<(), BoxError> {
    remove_profile_at(&profile_path()?)
}

fn read_profile_at(path: &Path) -> Result<Option<OfflineProfile>, BoxError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let profile: OfflineProfile = serde_json::from_slice(&bytes)?;
    Ok(Some(OfflineProfile::new(&profile.name)?))
}

fn save_profile_at(path: &Path, profile: &OfflineProfile) -> Result<(), BoxError> {
    let directory = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Offline profile path has no parent",
        )
    })?;
    fs::create_dir_all(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer_pretty(&mut temporary, profile)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn remove_profile_at(path: &Path) -> Result<(), BoxError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn saved_active_profile_can_be_replaced_and_removed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config").join("offline-profile.json");
        assert_eq!(read_profile_at(&path).unwrap(), None);
        for name in ["Player_One", "Player_Two"] {
            let profile = OfflineProfile::new(name).unwrap();
            save_profile_at(&path, &profile).unwrap();
            assert_eq!(read_profile_at(&path).unwrap(), Some(profile));
        }
        remove_profile_at(&path).unwrap();
        remove_profile_at(&path).unwrap();
        assert_eq!(read_profile_at(&path).unwrap(), None);
    }
}

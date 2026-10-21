// SPDX-License-Identifier: MIT
//! Local launcher defaults and per-game runtime overrides.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

pub type PreferencesError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum JavaOverride {
    #[default]
    Inherit,
    Automatic,
    Custom {
        executable: PathBuf,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchOverrides {
    #[serde(default)]
    pub java: JavaOverride,
    #[serde(default)]
    pub memory_mb: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherPreferences {
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    #[serde(default)]
    pub game_directory: Option<PathBuf>,
}

const fn default_memory_mb() -> u32 {
    2048
}

impl Default for LauncherPreferences {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            game_directory: None,
        }
    }
}

pub fn validate_memory_mb(memory_mb: u32) -> Result<(), PreferencesError> {
    if !(512..=262144).contains(&memory_mb) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Maximum memory must be between 512 and 262144 MiB",
        )
        .into());
    }
    Ok(())
}

fn preferences_path() -> Result<PathBuf, PreferencesError> {
    Ok(dirs::config_dir()
        .ok_or("could not locate the user config directory")?
        .join("CobbleLauncher/launcher-preferences.json"))
}

pub fn read_launcher_preferences() -> Result<LauncherPreferences, PreferencesError> {
    read_preferences_at(&preferences_path()?)
}

pub fn save_launcher_preferences(
    preferences: &LauncherPreferences,
) -> Result<(), PreferencesError> {
    save_preferences_at(&preferences_path()?, preferences)
}

fn read_preferences_at(path: &Path) -> Result<LauncherPreferences, PreferencesError> {
    let preferences: LauncherPreferences = match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => LauncherPreferences::default(),
        Err(error) => return Err(error.into()),
    };
    validate_memory_mb(preferences.memory_mb)?;
    Ok(preferences)
}

fn save_preferences_at(
    path: &Path,
    preferences: &LauncherPreferences,
) -> Result<(), PreferencesError> {
    validate_memory_mb(preferences.memory_mb)?;
    let parent = path
        .parent()
        .ok_or("launcher preferences path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut temporary, preferences)?;
    temporary.write_all(b"\n")?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_override_serialization_are_backwards_compatible() {
        assert_eq!(
            serde_json::from_str::<LaunchOverrides>("{}").unwrap(),
            LaunchOverrides::default()
        );
        assert_eq!(
            serde_json::from_str::<LauncherPreferences>("{}").unwrap(),
            LauncherPreferences::default()
        );
        for java in [
            JavaOverride::Inherit,
            JavaOverride::Automatic,
            JavaOverride::Custom {
                executable: PathBuf::from("C:/Java folder/bin/java.exe"),
            },
        ] {
            let options = LaunchOverrides {
                java,
                memory_mb: Some(4096),
            };
            assert_eq!(
                serde_json::from_slice::<LaunchOverrides>(&serde_json::to_vec(&options).unwrap())
                    .unwrap(),
                options
            );
        }
        assert!(serde_json::from_str::<JavaOverride>(r#"{"mode":"custom"}"#).is_err());
        assert!(serde_json::from_str::<JavaOverride>(r#"{"mode":"unknown"}"#).is_err());
    }

    #[test]
    fn preferences_reload_and_invalid_save_preserves_old_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config/launcher-preferences.json");
        assert_eq!(
            read_preferences_at(&path).unwrap(),
            LauncherPreferences::default()
        );
        let preferences = LauncherPreferences {
            memory_mb: 4096,
            game_directory: Some(directory.path().join("Games folder")),
        };
        save_preferences_at(&path, &preferences).unwrap();
        assert_eq!(read_preferences_at(&path).unwrap(), preferences);
        let mut invalid = preferences.clone();
        invalid.memory_mb = 511;
        assert!(save_preferences_at(&path, &invalid).is_err());
        assert_eq!(read_preferences_at(&path).unwrap(), preferences);
        fs::write(&path, br#"{"memory_mb":0}"#).unwrap();
        assert!(read_preferences_at(&path).is_err());
        for value in [0, 511, 262145, u32::MAX] {
            assert!(validate_memory_mb(value).is_err());
        }
        for value in [512, 2048, 262144] {
            assert!(validate_memory_mb(value).is_ok());
        }
    }
}

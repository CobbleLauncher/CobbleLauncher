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
    #[serde(default)]
    pub window_size: Option<WindowSize>,
    #[serde(default)]
    pub jvm_args: Option<Vec<String>>,
    #[serde(default)]
    pub launch_behavior: Option<LaunchBehavior>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSize {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchBehavior {
    #[default]
    Keep,
    Minimize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherPreferences {
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    #[serde(default)]
    pub game_directory: Option<PathBuf>,
    #[serde(default)]
    pub window_size: WindowSize,
    #[serde(default)]
    pub jvm_args: Vec<String>,
    #[serde(default)]
    pub launch_behavior: LaunchBehavior,
}

const fn default_memory_mb() -> u32 {
    2048
}

impl Default for LauncherPreferences {
    fn default() -> Self {
        Self {
            memory_mb: default_memory_mb(),
            game_directory: None,
            window_size: WindowSize::default(),
            jvm_args: Vec::new(),
            launch_behavior: LaunchBehavior::Keep,
        }
    }
}

pub fn validate_advanced_runtime(
    window: &WindowSize,
    arguments: &[String],
) -> Result<(), PreferencesError> {
    if window
        .width
        .is_some_and(|value| !(320..=16384).contains(&value))
        || window
            .height
            .is_some_and(|value| !(240..=16384).contains(&value))
    {
        return Err("Window width must be 320-16384 and height 240-16384".into());
    }
    if arguments.len() > 128 {
        return Err("At most 128 extra JVM arguments are allowed".into());
    }
    for argument in arguments {
        if argument.len() > 8192
            || !argument.starts_with('-')
            || argument.contains(['\0', '\n', '\r'])
            || [
                "-cp",
                "-classpath",
                "--class-path",
                "-jar",
                "-m",
                "--module",
                "--",
            ]
            .contains(&argument.as_str())
            || argument.starts_with("-Xmx")
            || argument.starts_with("--class-path=")
            || argument.starts_with("--module=")
        {
            return Err("Extra JVM arguments must be individual options; memory, classpath and main class are launcher-managed".into());
        }
    }
    Ok(())
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
    validate_advanced_runtime(&preferences.window_size, &preferences.jvm_args)?;
    Ok(preferences)
}

fn save_preferences_at(
    path: &Path,
    preferences: &LauncherPreferences,
) -> Result<(), PreferencesError> {
    validate_memory_mb(preferences.memory_mb)?;
    validate_advanced_runtime(&preferences.window_size, &preferences.jvm_args)?;
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
    fn advanced_defaults_and_explicit_empty_overrides_remain_distinct() {
        let legacy: LauncherPreferences = serde_json::from_str(r#"{"memory_mb":4096}"#).unwrap();
        assert_eq!(legacy.window_size, WindowSize::default());
        assert!(legacy.jvm_args.is_empty());
        assert_eq!(legacy.launch_behavior, LaunchBehavior::Keep);
        let overrides: LaunchOverrides =
            serde_json::from_str(r#"{"window_size":{},"jvm_args":[],"launch_behavior":"keep"}"#)
                .unwrap();
        assert_eq!(overrides.window_size, Some(WindowSize::default()));
        assert_eq!(overrides.jvm_args, Some(vec![]));
        assert_ne!(overrides, LaunchOverrides::default());
        for argument in [
            "-Xmx8G",
            "-cp",
            "--class-path=other.jar",
            "@options",
            "MainClass",
            "-Dfoo=bad\nvalue",
        ] {
            assert!(validate_advanced_runtime(&WindowSize::default(), &[argument.into()]).is_err());
        }
        assert!(
            validate_advanced_runtime(
                &WindowSize {
                    width: Some(319),
                    height: None
                },
                &[]
            )
            .is_err()
        );
        assert!(
            validate_advanced_runtime(
                &WindowSize {
                    width: Some(1280),
                    height: Some(720)
                },
                &["-Dlabel=two words".into()]
            )
            .is_ok()
        );
    }

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
                ..LaunchOverrides::default()
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
            ..LauncherPreferences::default()
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

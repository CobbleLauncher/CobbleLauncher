// SPDX-License-Identifier: MIT
use crate::{JavaOverride, JavaRuntime, find_java, probe_java};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::PathBuf,
};

type SettingsError = Box<dyn Error + Send + Sync>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct JavaSettings {
    pub executable: Option<PathBuf>,
}

fn settings_path() -> Result<PathBuf, SettingsError> {
    Ok(dirs::config_dir()
        .ok_or("could not locate the user config directory")?
        .join("CobbleLauncher/java.json"))
}

pub fn read_java_settings() -> Result<JavaSettings, SettingsError> {
    match fs::read(settings_path()?) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(JavaSettings::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn save_java_settings(settings: &JavaSettings) -> Result<(), SettingsError> {
    let path = settings_path()?;
    let parent = path
        .parent()
        .expect("Java settings have a parent directory");
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(&serde_json::to_vec_pretty(settings)?)?;
    file.persist(path)?;
    Ok(())
}

/// Resolve the saved preference only when preparing a game, not on startup.
pub async fn resolve_java(required_major: u32) -> Result<JavaRuntime, SettingsError> {
    resolve_java_override(required_major, &JavaOverride::Inherit).await
}

/// An automatic game override deliberately bypasses the saved custom runtime.
pub async fn resolve_java_override(
    required_major: u32,
    choice: &JavaOverride,
) -> Result<JavaRuntime, SettingsError> {
    let selected = selected_java_path(choice, || Ok(read_java_settings()?.executable))?;
    let runtime = match selected {
        Some(path) => probe_java(&path).await?,
        None => find_java(required_major).await?,
    };
    validate_runtime(required_major, runtime)
}

pub fn selected_java_path(
    choice: &JavaOverride,
    inherited: impl FnOnce() -> Result<Option<PathBuf>, SettingsError>,
) -> Result<Option<PathBuf>, SettingsError> {
    match choice {
        JavaOverride::Inherit => inherited(),
        JavaOverride::Automatic => Ok(None),
        JavaOverride::Custom { executable } if executable.as_os_str().is_empty() => {
            Err("Choose a Java executable".into())
        }
        JavaOverride::Custom { executable } => Ok(Some(executable.clone())),
    }
}

fn validate_runtime(
    required_major: u32,
    runtime: JavaRuntime,
) -> Result<JavaRuntime, SettingsError> {
    if runtime.major != required_major {
        return Err(format!(
            "Minecraft requires Java {required_major}; selected Java is {}",
            runtime.major
        )
        .into());
    }
    let matches_arch = match std::env::consts::ARCH {
        "x86_64" => matches!(runtime.arch.as_str(), "amd64" | "x86_64" | "x64"),
        "aarch64" => matches!(runtime.arch.as_str(), "aarch64" | "arm64"),
        "x86" => matches!(runtime.arch.as_str(), "x86" | "i386" | "i686"),
        _ => false,
    };
    if !matches_arch {
        return Err(format!(
            "Java architecture {} does not match {}",
            runtime.arch,
            std::env::consts::ARCH
        )
        .into());
    }
    Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_and_automatic_runtimes_share_version_and_architecture_validation() {
        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "aarch64",
            "x86" => "x86",
            _ => return,
        };
        let runtime = JavaRuntime {
            executable: PathBuf::from("java"),
            version: "21.0.1".into(),
            major: 21,
            arch: arch.into(),
            vendor: "Synthetic runtime".into(),
        };
        assert!(validate_runtime(21, runtime.clone()).is_ok());
        assert!(validate_runtime(17, runtime.clone()).is_err());
        let mut wrong_arch = runtime;
        wrong_arch.arch = "unsupported-cpu".into();
        assert!(validate_runtime(21, wrong_arch).is_err());
    }

    #[test]
    fn game_java_overrides_only_read_global_settings_when_inheriting() {
        let inherited = PathBuf::from("global-java");
        assert_eq!(
            selected_java_path(&JavaOverride::Inherit, || Ok(Some(inherited.clone()))).unwrap(),
            Some(inherited)
        );
        assert_eq!(
            selected_java_path(&JavaOverride::Automatic, || Err(
                "global settings unavailable".into()
            ))
            .unwrap(),
            None
        );
        let explicit = PathBuf::from("per-game-java");
        assert_eq!(
            selected_java_path(
                &JavaOverride::Custom {
                    executable: explicit.clone()
                },
                || Err("must not read global".into())
            )
            .unwrap(),
            Some(explicit)
        );
        assert!(
            selected_java_path(
                &JavaOverride::Custom {
                    executable: PathBuf::new()
                },
                || Ok(None)
            )
            .is_err()
        );
    }
}

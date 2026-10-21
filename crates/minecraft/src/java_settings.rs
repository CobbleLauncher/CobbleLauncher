// SPDX-License-Identifier: MIT
use crate::{JavaRuntime, find_java, probe_java};
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
    let runtime = match read_java_settings()?.executable {
        Some(path) => probe_java(&path).await?,
        None => find_java(required_major).await?,
    };
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

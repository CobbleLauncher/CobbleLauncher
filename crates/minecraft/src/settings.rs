// SPDX-License-Identifier: MIT
//! Small local preference shared by the desktop host and CLI. No network I/O.
use crate::{DownloadSource, SourceError};
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSourceChoice {
    #[default]
    Official,
    Bmclapi,
    Custom,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DownloadSettings {
    pub source: DownloadSourceChoice,
    pub mirror: String,
}
impl DownloadSettings {
    pub fn download_source(&self) -> Result<DownloadSource, SourceError> {
        match self.source {
            DownloadSourceChoice::Official => Ok(DownloadSource::Official),
            DownloadSourceChoice::Bmclapi => Ok(DownloadSource::Bmclapi),
            DownloadSourceChoice::Custom => DownloadSource::custom(self.mirror.trim()),
        }
    }
}

#[derive(Debug)]
pub enum DownloadSettingsError {
    ConfigDirectoryUnavailable,
    Io(io::Error),
    Json(serde_json::Error),
    InvalidSource(SourceError),
}
impl fmt::Display for DownloadSettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConfigDirectoryUnavailable => f.write_str("user config directory unavailable"),
            Self::Io(_) => f.write_str("could not read or save download settings"),
            Self::Json(_) => f.write_str("invalid download settings JSON"),
            Self::InvalidSource(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DownloadSettingsError {}
impl From<io::Error> for DownloadSettingsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
fn settings_path() -> Result<PathBuf, DownloadSettingsError> {
    Ok(dirs::config_dir()
        .ok_or(DownloadSettingsError::ConfigDirectoryUnavailable)?
        .join("CobbleLauncher")
        .join("download-source.json"))
}
pub fn read_download_settings() -> Result<DownloadSettings, DownloadSettingsError> {
    read_at(&settings_path()?)
}
pub fn save_download_settings(settings: &DownloadSettings) -> Result<(), DownloadSettingsError> {
    save_at(&settings_path()?, settings)
}
fn read_at(path: &Path) -> Result<DownloadSettings, DownloadSettingsError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(DownloadSettings::default());
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(16_385).read_to_end(&mut bytes)?;
    if bytes.len() > 16_384 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "settings too large").into());
    }
    let settings: DownloadSettings =
        serde_json::from_slice(&bytes).map_err(DownloadSettingsError::Json)?;
    settings
        .download_source()
        .map_err(DownloadSettingsError::InvalidSource)?;
    Ok(settings)
}
fn save_at(path: &Path, settings: &DownloadSettings) -> Result<(), DownloadSettingsError> {
    settings
        .download_source()
        .map_err(DownloadSettingsError::InvalidSource)?;
    let bytes = serde_json::to_vec_pretty(settings).map_err(DownloadSettingsError::Json)?;
    if bytes.len() > 16_384 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "settings too large").into());
    }
    let parent = path
        .parent()
        .ok_or(DownloadSettingsError::ConfigDirectoryUnavailable)?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| DownloadSettingsError::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_defaults_and_saved_choice_survives_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        assert_eq!(read_at(&path).unwrap(), DownloadSettings::default());
        for source in [
            DownloadSourceChoice::Bmclapi,
            DownloadSourceChoice::Custom,
            DownloadSourceChoice::Official,
        ] {
            let settings = DownloadSettings {
                source,
                mirror: "https://mirror.example.invalid/minecraft/".into(),
            };
            save_at(&path, &settings).unwrap();
            assert_eq!(read_at(&path).unwrap(), settings);
        }
    }
    #[test]
    fn invalid_input_never_replaces_a_saved_choice() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        save_at(&path, &DownloadSettings::default()).unwrap();
        for mirror in [
            "",
            "http://example.invalid",
            "https://user:secret@example.invalid",
            "https://example.invalid/?secret=x",
        ] {
            assert!(
                save_at(
                    &path,
                    &DownloadSettings {
                        source: DownloadSourceChoice::Custom,
                        mirror: mirror.into()
                    }
                )
                .is_err()
            );
            assert_eq!(read_at(&path).unwrap(), DownloadSettings::default());
        }
        fs::write(&path, br#"{"source":"unknown","mirror":""}"#).unwrap();
        assert!(read_at(&path).is_err());
        fs::write(&path, vec![b' '; 16_385]).unwrap();
        assert!(read_at(&path).is_err());
    }
}

// SPDX-License-Identifier: MIT
//! Validated global network budgets. This does not increase the disk-worker budget.
use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadTuning {
    #[serde(default = "adaptive_default")]
    pub adaptive: bool,
    pub max_concurrent: usize,
    pub per_origin: usize,
}
impl Default for DownloadTuning {
    fn default() -> Self {
        Self {
            adaptive: true,
            max_concurrent: 12,
            per_origin: 4,
        }
    }
}
fn adaptive_default() -> bool {
    true
}
impl DownloadTuning {
    pub fn validate(self) -> Result<Self, crate::NetError> {
        if !(1..=32).contains(&self.max_concurrent)
            || !(1..=8).contains(&self.per_origin)
            || self.per_origin > self.max_concurrent
        {
            return Err(crate::NetError::InvalidConfig);
        }
        Ok(self)
    }
}
fn path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|root| root.join("CobbleLauncher").join("download-tuning.json"))
}
fn state() -> &'static RwLock<DownloadTuning> {
    static STATE: OnceLock<RwLock<DownloadTuning>> = OnceLock::new();
    STATE.get_or_init(|| {
        RwLock::new(
            path()
                .and_then(|path| std::fs::read(path).ok())
                .and_then(|bytes| serde_json::from_slice::<DownloadTuning>(&bytes).ok())
                .and_then(|value| value.validate().ok())
                .unwrap_or_default(),
        )
    })
}
pub fn download_tuning() -> DownloadTuning {
    state().read().map(|value| *value).unwrap_or_default()
}
/// Applies to newly created brokers/managers; active workflows keep their budgets.
pub fn save_download_tuning(value: DownloadTuning) -> std::io::Result<()> {
    let value = value.validate().map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid download concurrency",
        )
    })?;
    let path =
        path().ok_or_else(|| std::io::Error::other("configuration directory unavailable"))?;
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| std::io::Error::other("configuration directory unavailable"))?,
    )?;
    std::fs::write(path, serde_json::to_vec(&value)?)?;
    *state()
        .write()
        .map_err(|_| std::io::Error::other("download configuration unavailable"))? = value;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tuning_bounds_and_default_are_explicit() {
        assert!(DownloadTuning::default().adaptive);
        let legacy: DownloadTuning =
            serde_json::from_str(r#"{"max_concurrent":6,"per_origin":2}"#).unwrap();
        assert!(legacy.adaptive);
        assert_eq!(legacy.max_concurrent, 6);
        assert_eq!(legacy.per_origin, 2);
        assert_eq!(DownloadTuning::default().max_concurrent, 12);
        assert_eq!(DownloadTuning::default().per_origin, 4);
        for value in [
            DownloadTuning {
                adaptive: true,
                max_concurrent: 0,
                per_origin: 1,
            },
            DownloadTuning {
                adaptive: true,
                max_concurrent: 33,
                per_origin: 4,
            },
            DownloadTuning {
                adaptive: true,
                max_concurrent: 12,
                per_origin: 9,
            },
            DownloadTuning {
                adaptive: true,
                max_concurrent: 1,
                per_origin: 2,
            },
        ] {
            assert!(value.validate().is_err());
        }
        assert!(
            DownloadTuning {
                adaptive: true,
                max_concurrent: 32,
                per_origin: 8
            }
            .validate()
            .is_ok()
        );
    }
}

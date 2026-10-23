// SPDX-License-Identifier: MIT
//! Read-only host inspection envelope; the scope is a closed permission boundary.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InspectionScope {
    Mods,
    Packs,
    Worlds,
    Backups,
    Screenshots,
    Storage,
    Runtime,
    Options,
    Datapacks,
    Servers,
}
impl InspectionScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mods => "mods",
            Self::Packs => "packs",
            Self::Worlds => "worlds",
            Self::Backups => "backups",
            Self::Screenshots => "screenshots",
            Self::Storage => "storage",
            Self::Runtime => "runtime",
            Self::Options => "options",
            Self::Datapacks => "datapacks",
            Self::Servers => "servers",
        }
    }
}
pub const MAX_INSPECTION_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionGame {
    pub id: String,
    pub name: String,
    pub version: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionSnapshot {
    pub schema_version: u32,
    pub scope: InspectionScope,
    pub game: InspectionGame,
    pub data: serde_json::Value,
    pub revision: String,
}

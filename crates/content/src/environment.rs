// SPDX-License-Identifier: MIT
//! Joint provider-aware environment plans. Credentials and acquisition policies
//! stay with each provider; matching display names never merges identities.
pub(crate) mod catalog;
mod reverse;
pub use reverse::{resolve_environment_selected, reverse_configure_environment};

use crate::{ModrinthPlan, ReverseConfigurationStats, curseforge};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvironmentSource {
    Modrinth,
    Curseforge,
}
impl EnvironmentSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvironmentProject {
    pub source: EnvironmentSource,
    pub project_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentRoot {
    pub source: EnvironmentSource,
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvironmentSelection {
    pub source: EnvironmentSource,
    pub project_id: String,
    pub version_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentDependencyEdge {
    pub from: EnvironmentSelection,
    pub to: EnvironmentSelection,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentPlan {
    pub game_version: String,
    pub loader: String,
    pub roots: Vec<EnvironmentSelection>,
    pub modrinth: Option<ModrinthPlan>,
    pub curseforge: Option<curseforge::InstallPlan>,
    #[serde(default)]
    pub dependency_edges: Vec<EnvironmentDependencyEdge>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseEnvironmentChoice {
    pub game_version: String,
    pub loader: String,
    pub roots: Vec<EnvironmentSelection>,
    pub plan: EnvironmentPlan,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvironmentConflict {
    pub game_version: String,
    pub loader: String,
    pub code: String,
    pub message: String,
    pub projects: Vec<EnvironmentProject>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReverseEnvironmentConfiguration {
    pub choices: Vec<ReverseEnvironmentChoice>,
    pub conflicts: Vec<EnvironmentConflict>,
    pub stats: ReverseConfigurationStats,
    pub limitations: Vec<String>,
}

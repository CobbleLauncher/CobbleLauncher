// SPDX-License-Identifier: MIT
//! Application use cases and orchestration. Independent of Tauri and the UI.
//!
//! Application bootstrap does not initialize network, storage or plugins.

use cobble_domain::AppInfo;

mod cache;
mod fabric;
mod install;
mod launch;
mod modrinth;
mod runner;
pub use fabric::{FabricVersion, install_fabric, list_fabric_versions, list_fabric_versions_from};
pub use install::{InstallOutcome, InstallProgress, install_game, install_resources};
pub use launch::{LaunchPreparation, prepare_launch};
pub use modrinth::{ModrinthInstallOutcome, install_modrinth_plan};
pub use runner::{RunningGame, start_offline_game};

/// Local, side-effect-free information shared by desktop and CLI adapters.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Cobble Launcher",
        version: env!("CARGO_PKG_VERSION"),
        stage: "development",
    }
}

// SPDX-License-Identifier: MIT
//! Application use cases and orchestration. Independent of Tauri and the UI.
//!
//! Application bootstrap does not initialize network, storage or plugins.

use cobble_domain::AppInfo;

mod cache;
mod fabric;
mod forge;
mod install;
mod instances;
mod launch;
mod modrinth;
mod modupdates;
mod mrpack;
mod runner;
pub use fabric::{FabricVersion, install_fabric, list_fabric_versions, list_fabric_versions_from};
pub use forge::{
    ForgeKind, ForgeVersion, install_forge, list_forge_versions, list_forge_versions_from,
};
pub use install::{InstallOutcome, InstallProgress, install_game, install_resources};
pub use instances::{copy_instance, export_instance_mrpack};
pub use launch::{LaunchPreparation, prepare_launch};
pub use modrinth::{ModrinthInstallOutcome, install_modrinth_plan};
pub use modupdates::{
    ModUpdateCandidate, ModUpdateChange, ModUpdateOutcome, ModUpdatePlan, apply_modrinth_updates,
    check_modrinth_updates, managed_mod_count, prepare_modrinth_updates,
};
pub use mrpack::{MrpackInstallOutcome, install_mrpack};
pub use runner::{
    RunningGame, prepare_offline_arguments, start_offline_game, start_offline_game_with_options,
};

/// Local, side-effect-free information shared by desktop and CLI adapters.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Cobble Launcher",
        version: env!("CARGO_PKG_VERSION"),
        stage: "development",
    }
}

// SPDX-License-Identifier: MIT
//! Application use cases and orchestration. Independent of Tauri and the UI.
//!
//! Application bootstrap does not initialize network, storage or plugins.

use cobble_domain::AppInfo;

mod cache;
mod content_pack_install;
mod control;
mod curseforge;
mod fabric;
mod foreign_import;
mod forge;
mod install;
mod instances;
mod launch;
mod modrecognition;
mod modrinth;
mod modupdates;
pub use curseforge::{
    import_curseforge_manual_file, install_curseforge_plan_controlled,
    missing_curseforge_manual_files,
};
mod curseforge_pack;
mod mrpack;
pub use curseforge_pack::{
    CurseForgePackInspection, CurseForgePackPlan, curseforge_pack_file_path, curseforge_pack_pins,
    download_curseforge_pack_archive_controlled, import_curseforge_pack_archive,
    import_curseforge_pack_manual_file, inspect_curseforge_pack,
    install_curseforge_pack_controlled, missing_curseforge_pack_manual_files,
    validate_curseforge_pack_plan,
};
mod pack_download;
mod runner;
mod world_restore;
mod worlds;
pub use content_pack_install::{
    install_modrinth_content_pack, install_modrinth_content_pack_controlled,
};
pub use control::{OperationCancelled, OperationControl};
pub use fabric::{
    FabricVersion, install_fabric, install_fabric_controlled, list_fabric_versions,
    list_fabric_versions_from,
};
pub use foreign_import::{
    LauncherContentRoot, LauncherImportCandidate, import_launcher_game,
    import_launcher_game_controlled, inspect_launcher_import, resume_launcher_import,
    resume_launcher_import_controlled,
};
pub use forge::{
    ForgeKind, ForgeVersion, install_forge, install_forge_controlled, list_forge_versions,
    list_forge_versions_from,
};
pub use install::{
    DownloadProgressDetail, InstallOutcome, InstallProgress, install_game, install_game_controlled,
    install_resources, install_resources_controlled,
};
pub use instances::{copy_instance, export_instance_mrpack};
pub use launch::{LaunchPreparation, prepare_launch};
pub use modrecognition::{ModRecognitionOutcome, recognize_modrinth_mods};
pub use modrinth::{
    ModrinthInstallOutcome, install_modrinth_plan, install_modrinth_plan_controlled,
};
pub use modupdates::{
    ModOperationHistory, ModOperationFile, list_mod_operation_history, undo_mod_operation, preview_mod_operation_undo,
    ManagedModSummary, ModUpdateCandidate, ModUpdateChange, ModUpdateOutcome, ModUpdatePlan,
    apply_curseforge_updates_controlled, apply_modrinth_updates, apply_modrinth_updates_controlled,
    check_modrinth_updates, list_managed_curseforge_mods, list_managed_modrinth_mods,
    managed_mod_count, prepare_curseforge_update, prepare_modrinth_updates,
    prepare_modrinth_version_changes, recover_modrinth_update,
};
pub use mrpack::{MrpackInstallOutcome, install_mrpack, install_mrpack_controlled};
pub use pack_download::{download_modrinth_pack, download_modrinth_pack_controlled};
mod server_launch;
pub use server_launch::{direct_connect_arguments, supports_direct_connect};
pub use runner::{
    RunningGame, prepare_identity_arguments, prepare_identity_arguments_with_runtime,
    prepare_offline_arguments, start_game_with_identity, start_game_with_identity_and_server, start_offline_game,
    start_offline_game_with_options,
};
pub use world_restore::{
    WorldImportOutcome, WorldReplacementBackup, import_world_archive,
    list_world_replacement_backups, recover_world_operations, restore_world_replacement_backup,
    world_operation_is_complete,
};
pub use worlds::export_world_backup;
mod world_snapshots;
pub use world_snapshots::{
    create_automatic_world_snapshot_controlled,
    RetentionPlan, RetentionPolicy, RetentionPreview, RetentionResult, WorldSnapshot,
    WorldSnapshotRepository, create_world_snapshot, create_world_snapshot_controlled,
    restore_world_snapshot, world_snapshot_identity,
};

/// Local, side-effect-free information shared by desktop and CLI adapters.
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Cobble Launcher",
        version: env!("CARGO_PKG_VERSION"),
        stage: "development",
    }
}

pub use cobble_backup::{BackupControl, RepositoryInsights, SnapshotComparison};

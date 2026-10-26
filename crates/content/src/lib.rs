// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod compatibility;
pub mod dependency_solver;
pub mod environment;
pub use compatibility::{Compatibility, CompatibilityStatus, query_compatibility};
pub use environment::{
    EnvironmentConflict, EnvironmentDependencyEdge, EnvironmentPlan, EnvironmentProject,
    EnvironmentRoot, EnvironmentSelection, EnvironmentSource, ReverseEnvironmentChoice,
    ReverseEnvironmentConfiguration, resolve_environment_selected, reverse_configure_environment,
};
pub mod curseforge;
mod curseforge_pack;
mod local_mods;
mod local_packs;
pub mod media;
mod pack_metadata;
pub use pack_metadata::{MAX_PACK_ARCHIVE_BYTES, MAX_PACK_METADATA_BYTES, read_pack_metadata_zip};
mod modrinth;
mod modrinth_content_packs;
mod modrinth_packs;
mod mrpack;
mod mrpack_inspection;
pub mod pack_update_preview;
mod world_archives;
mod world_seed;
mod worlds;
pub use world_seed::{
    LocalWorldMetadata, WorldGeneratorMetadata, WorldSpawn, local_world_metadata,
    read_saved_world_metadata,
};

pub use curseforge_pack::{
    CurseForgePack, CurseForgePackFile, CurseForgePackLoader, read_curseforge_pack,
    read_curseforge_pack_file, validate_curseforge_pack_paths,
};
pub use mrpack::{Mrpack, MrpackFile, PackOverride, read_mrpack, read_mrpack_file};
pub use mrpack_inspection::{
    MrpackInspectionOverride, MrpackInspectionSnapshot, inspect_mrpack_archive,
};
pub use world_archives::{
    MAX_WORLD_ARCHIVE_BYTES, WorldArchiveEntry, WorldArchivePlan, WorldArchivePreview,
    inspect_world_archive, preview_world_archive,
};
pub use worlds::{LocalWorld, list_local_worlds, read_world_metadata, world_directory};

pub use local_mods::{BoxError, LocalMod, import_mods, list_mods, remove_mod, set_mod_enabled};
pub use local_packs::{
    LocalPack, LocalPackKind, import_local_packs, list_local_packs, remove_local_pack,
};
pub use modrinth::{
    IdentifiedMod, ModProject, ModVersion, ModVersionDetails, ModVersionSelection, ModrinthFile,
    ModrinthPlan, ModrinthUpdate, ResolvedMod, ReverseConfiguration, ReverseConfigurationChoice,
    ReverseConfigurationConflict, ReverseConfigurationStats, SearchResults, find_modrinth_update,
    identify_modrinth_file, identify_modrinth_file_with_broker, list_modrinth_versions,
    modrinth_version_details, modrinth_version_number, resolve_modrinth, resolve_modrinth_many,
    resolve_modrinth_selected, reverse_configure_modrinth, search_modrinth,
    search_modrinth_unfiltered,
};
pub use modrinth_content_packs::{
    ContentPackVersion, ModrinthContentPackDownload, list_modrinth_content_pack_versions,
    resolve_modrinth_content_pack, search_modrinth_content_packs,
};
pub use modrinth_packs::{
    ModrinthPackDownload, PackVersion, list_modrinth_pack_versions, resolve_modrinth_pack,
    search_modrinth_packs, supported_pack_target,
};

mod world_datapacks;
pub use world_datapacks::{
    WorldDatapacks, import_world_datapacks, list_world_datapacks, move_world_datapack,
};

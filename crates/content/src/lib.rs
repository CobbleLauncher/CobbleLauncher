// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod local_mods;
mod local_packs;
mod modrinth;
mod modrinth_content_packs;
mod modrinth_packs;
mod mrpack;
mod world_archives;
mod worlds;

pub use mrpack::{Mrpack, MrpackFile, PackOverride, read_mrpack, read_mrpack_file};
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
    ModrinthPlan, ModrinthUpdate, ResolvedMod, SearchResults, find_modrinth_update,
    identify_modrinth_file, identify_modrinth_file_with_broker, list_modrinth_versions,
    modrinth_version_details, modrinth_version_number, resolve_modrinth, resolve_modrinth_many,
    resolve_modrinth_selected, search_modrinth,
};
pub use modrinth_content_packs::{
    ContentPackVersion, ModrinthContentPackDownload, list_modrinth_content_pack_versions,
    resolve_modrinth_content_pack, search_modrinth_content_packs,
};
pub use modrinth_packs::{
    ModrinthPackDownload, PackVersion, list_modrinth_pack_versions, resolve_modrinth_pack,
    search_modrinth_packs, supported_pack_target,
};

// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod local_mods;
mod local_packs;
mod modrinth;
mod modrinth_packs;
mod mrpack;

pub use mrpack::{Mrpack, MrpackFile, PackOverride, read_mrpack, read_mrpack_file};

pub use local_mods::{BoxError, LocalMod, import_mods, list_mods, remove_mod, set_mod_enabled};
pub use local_packs::{
    LocalPack, LocalPackKind, import_local_packs, list_local_packs, remove_local_pack,
};
pub use modrinth::{
    IdentifiedMod, ModProject, ModrinthFile, ModrinthPlan, ModrinthUpdate, ResolvedMod,
    SearchResults, find_modrinth_update, identify_modrinth_file,
    identify_modrinth_file_with_broker, modrinth_version_number, resolve_modrinth,
    resolve_modrinth_many, search_modrinth,
};
pub use modrinth_packs::{
    ModrinthPackDownload, PackVersion, list_modrinth_pack_versions, resolve_modrinth_pack,
    search_modrinth_packs, supported_pack_target,
};

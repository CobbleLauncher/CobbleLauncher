// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod local_mods;
mod modrinth;
mod mrpack;

pub use mrpack::{Mrpack, MrpackFile, PackOverride, read_mrpack, read_mrpack_file};

pub use local_mods::{BoxError, LocalMod, import_mods, list_mods, remove_mod, set_mod_enabled};
pub use modrinth::{
    ModProject, ModrinthFile, ModrinthPlan, ResolvedMod, SearchResults, resolve_modrinth,
    search_modrinth,
};

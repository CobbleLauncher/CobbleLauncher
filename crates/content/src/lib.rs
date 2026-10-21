// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod local_mods;
mod modrinth;

pub use local_mods::{BoxError, LocalMod, import_mods, list_mods, remove_mod, set_mod_enabled};
pub use modrinth::{
    ModProject, ModrinthFile, ModrinthPlan, ResolvedMod, SearchResults, resolve_modrinth,
    search_modrinth,
};

// SPDX-License-Identifier: MIT
//! Local mod files and their optional descriptive metadata.

mod local_mods;

pub use local_mods::{BoxError, LocalMod, import_mods, list_mods, remove_mod, set_mod_enabled};

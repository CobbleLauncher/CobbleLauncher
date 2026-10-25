// SPDX-License-Identifier: MIT
//! Operating-system paths, credentials, locks and process adapters.
//!
mod shortcut;
pub use shortcut::{create_launch_shortcut, read_shortcut_target};
mod recycle;
pub use recycle::{available_disk_space, recycle_directory};
mod disk;
pub use disk::disk_volume_identity;

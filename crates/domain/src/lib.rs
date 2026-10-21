// SPDX-License-Identifier: MIT
//! Pure domain models, IDs and errors. No I/O or framework dependencies.
//!
//! Renderer-safe application information. No credentials or local paths.

use serde::Serialize;

pub mod contracts;
pub mod generated;

/// Development bootstrap information, separate from persisted game models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub stage: &'static str,
}

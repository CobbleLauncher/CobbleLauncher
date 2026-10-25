// SPDX-License-Identifier: MIT
//! Host-derived shared runtime location. Discovery never downloads a runtime.
use std::path::PathBuf;
pub fn managed_java_root() -> Result<PathBuf, std::io::Error> {
    dirs::data_local_dir()
        .map(|root| root.join("CobbleLauncher/runtimes"))
        .ok_or_else(|| std::io::Error::other("managed Java data directory unavailable"))
}

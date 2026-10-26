// SPDX-License-Identifier: MIT
//! Paths fixed by the internal portable package and macOS application layouts.
use std::path::{Path, PathBuf};

pub(crate) fn plugin_host() -> Result<PathBuf, String> {
    helper(if cfg!(windows) {
        "cobble-plugin-host.exe"
    } else {
        "cobble-plugin-host"
    })
}

pub(crate) fn mesh_companion() -> Result<PathBuf, String> {
    helper(if cfg!(windows) {
        "cobble-mesh-companion.exe"
    } else {
        "cobble-mesh-companion"
    })
}

fn helper(name: &str) -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    helper_at(&executable, name)
}

fn helper_at(executable: &Path, name: &str) -> Result<PathBuf, String> {
    Ok(executable.parent().ok_or("无法找到程序目录")?.join(name))
}

pub(crate) fn bundled_plugins() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(resource_root_at(&executable, cfg!(target_os = "macos"))?.join("plugins"))
}

fn resource_root_at(executable: &Path, macos: bool) -> Result<PathBuf, String> {
    let directory = executable.parent().ok_or("无法找到资源目录")?;
    if macos
        && directory.file_name().is_some_and(|name| name == "MacOS")
        && let Some(contents) = directory.parent()
        && contents.file_name().is_some_and(|name| name == "Contents")
    {
        return Ok(contents.join("Resources"));
    }
    // Development and portable packages retain executable-relative resources.
    Ok(directory.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_portable_layout() {
        let executable = Path::new("portable/Cobble Launcher.exe");
        assert_eq!(
            helper_at(executable, "cobble-plugin-host.exe").unwrap(),
            Path::new("portable/cobble-plugin-host.exe")
        );
        assert_eq!(
            resource_root_at(executable, false).unwrap(),
            Path::new("portable")
        );
    }

    #[test]
    fn linux_portable_layout() {
        let executable = Path::new("/opt/cobble/cobble-desktop");
        assert_eq!(
            helper_at(executable, "cobble-plugin-host").unwrap(),
            Path::new("/opt/cobble/cobble-plugin-host")
        );
        assert_eq!(
            resource_root_at(executable, false).unwrap().join("plugins"),
            Path::new("/opt/cobble/plugins")
        );
    }

    #[test]
    fn macos_bundle_separates_helpers_and_resources() {
        let executable =
            Path::new("/Applications/Cobble Launcher.app/Contents/MacOS/cobble-desktop");
        assert_eq!(
            helper_at(executable, "cobble-plugin-host").unwrap(),
            Path::new("/Applications/Cobble Launcher.app/Contents/MacOS/cobble-plugin-host")
        );
        assert_eq!(
            resource_root_at(executable, true).unwrap().join("plugins"),
            Path::new("/Applications/Cobble Launcher.app/Contents/Resources/plugins")
        );
    }

    #[test]
    fn macos_unbundled_binary_keeps_portable_layout() {
        let executable = Path::new("/tmp/target/debug/cobble-desktop");
        assert_eq!(
            resource_root_at(executable, true).unwrap(),
            Path::new("/tmp/target/debug")
        );
        let executable = Path::new("/tmp/MacOS/cobble-desktop");
        assert_eq!(
            resource_root_at(executable, true).unwrap(),
            Path::new("/tmp/MacOS")
        );
    }
}

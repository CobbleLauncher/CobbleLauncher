// SPDX-License-Identifier: MIT
//! Lazy, verified shared Temurin acquisition. Explicit Java selections are never replaced.
mod archive;
mod install;
mod metadata;
pub(crate) async fn space_package(
    required: u32,
    broker: &cobble_net::Broker,
    control: &crate::OperationControl,
) -> Result<(u64, [u8; 32]), Error> {
    control.check_cancelled()?;
    let url = metadata::metadata_url(required)?;
    let location = cobble_minecraft::managed_java_root()?;
    match cap_std::fs::Dir::open_ambient_dir(location, cap_std::ambient_authority()) {
        Ok(root) => {
            if let Some(package) = cached_space_package(&root, required)? {
                return Ok(package);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let response = tokio::select! {value=broker.get(&url,None,1024*1024)=>value?,_=control.cancelled()=>return Err(crate::OperationCancelled.into())};
    control.check_cancelled()?;
    if response.metadata.status != 200
        || response.metadata.final_origin != "https://api.adoptium.net"
    {
        return Err("JAVA_METADATA_UNAVAILABLE".into());
    }
    let package = metadata::parse(&response.body, required)?;
    Ok((package.size, package.sha256))
}

fn cached_space_package(
    root: &cap_std::fs::Dir,
    required: u32,
) -> Result<Option<(u64, [u8; 32])>, Error> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    use std::io::Read;
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = match root.open_with(format!(".selection-{required}-x64.json"), &options) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Ok(None);
    }
    Ok(metadata::parse(&bytes, required)
        .ok()
        .map(|p| (p.size, p.sha256)))
}
type Error = Box<dyn std::error::Error + Send + Sync>;
use cobble_minecraft::{JavaOverride, JavaRuntime};
/// Read only installed metadata; does not extract natives or prepare a session.
pub fn launch_java_requirement(version: &str, directory: &std::path::Path) -> Result<u32, Error> {
    cobble_store::SafeRelativePath::parse(version)?;
    if version.contains('/') {
        return Err("invalid version identifier".into());
    }
    let bytes = std::fs::read(directory.join(format!("versions/{version}/{version}.json")))?;
    Ok(
        cobble_minecraft::VersionMetadata::parse_installed(&bytes, version)?
            .java_version
            .major_version,
    )
}
/// Freeze inheritance for this launch without persisting or changing user settings.
pub fn capture_java_choice(choice: &JavaOverride) -> Result<JavaOverride, Error> {
    Ok(
        match cobble_minecraft::selected_java_path(choice, || {
            Ok(cobble_minecraft::read_java_settings()?.executable)
        })? {
            Some(executable) => JavaOverride::Custom { executable },
            None => JavaOverride::Automatic,
        },
    )
}
pub async fn ensure_launch_java_controlled(
    required: u32,
    choice: &JavaOverride,
    control: crate::OperationControl,
) -> Result<JavaRuntime, Error> {
    control.check_cancelled()?;
    let choice = capture_java_choice(choice)?;
    let resolved = tokio::select! { result = cobble_minecraft::resolve_java_override(required, &choice) => result, _ = control.cancelled() => { control.check_cancelled()?; return Err("JAVA_CANCELLED".into()); } };
    control.check_cancelled()?;
    match resolved {
        Ok(runtime) => return Ok(runtime),
        Err(error)
            if matches!(choice, JavaOverride::Automatic)
                && matches!(
                    error.downcast_ref::<cobble_minecraft::JavaError>(),
                    Some(cobble_minecraft::JavaError::NotFound { .. })
                ) => {}
        Err(error) => return Err(error),
    }
    install_managed_java_controlled(required, control).await
}
/// Acquire only after an automatic selection has established that Java is absent.
pub async fn install_managed_java_controlled(
    required: u32,
    control: crate::OperationControl,
) -> Result<JavaRuntime, Error> {
    control.check_cancelled()?;
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err("JAVA_MANAGED_ACQUISITION_UNSUPPORTED_PLATFORM".into());
    }
    let root = cobble_minecraft::managed_java_root()?;
    let cache = dirs::cache_dir()
        .ok_or("Java cache directory unavailable")?
        .join("CobbleLauncher");
    install_managed_java_at_controlled(required, &root, &cache, control).await
}
/// Host-owned runtime/cache roots, also used by isolated acquisition probes.
/// These paths are never accepted from the renderer.
pub async fn install_managed_java_at_controlled(
    required: u32,
    root: &std::path::Path,
    cache: &std::path::Path,
    control: crate::OperationControl,
) -> Result<JavaRuntime, Error> {
    control.check_cancelled()?;
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err("JAVA_MANAGED_ACQUISITION_UNSUPPORTED_PLATFORM".into());
    }
    install::install_at(required, root, cache, control).await
}

#[cfg(test)]
#[path = "managed_java/space_tests.rs"]
mod space_tests;

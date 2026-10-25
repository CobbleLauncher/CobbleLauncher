// SPDX-License-Identifier: MIT
//! Explicit preflight reuses authoritative metadata and verified cache entries.
use crate::{
    OperationControl,
    modrinth::ModDownloads,
    readiness::{asset_files, file_matches},
    space::SpaceArtifact,
};
use cap_std::fs::Dir;
use cobble_content::EnvironmentPlan;
use cobble_minecraft::{DownloadSource, JavaOverride, PlatformContext, VersionCatalog};
use cobble_net::{Broker, BrokerConfig};
use sha1::{Digest, Sha1};
use std::path::Path;
type Error = Box<dyn std::error::Error + Send + Sync>;
#[path = "readiness_space/curseforge.rs"]
mod curseforge;
pub use curseforge::curseforge_pack_space_artifacts;

pub struct ArtifactSpacePlan {
    pub artifacts: Vec<SpaceArtifact>,
    pub required_java: u32,
    pub java_available: bool,
}

fn java_archive_demand(
    size: u64,
    cached: bool,
    cache_volume: &str,
    runtime_volume: &str,
) -> SpaceArtifact {
    SpaceArtifact {
        cache_volume: cache_volume.into(),
        target_volume: runtime_volume.into(),
        compressed_bytes: Some(size),
        // Extracted runtime/normalization output sizes remain unknown until ZIP inspection.
        installed_bytes: None,
        verified_cached: cached,
        already_installed: false,
        extraction_scratch_bytes: Some(size),
        backup_copy_bytes: 0,
    }
}
async fn java_archive_artifact(
    required: u32,
    broker: &Broker,
    cache: &ModDownloads,
    cache_volume: &str,
    runtime_volume: &str,
    control: &OperationControl,
) -> Result<SpaceArtifact, Error> {
    let (size, digest) = crate::managed_java::space_package(required, broker, control).await?;
    let expected = cobble_store::ArtifactExpectation {
        expected_size: Some(size),
        max_bytes: size,
        basis: cobble_store::IntegrityBasis::TrustedMetadata,
        digests: cobble_store::ExpectedDigests {
            sha256: Some(digest),
            ..Default::default()
        },
    };
    let cached = match cache.content.open_verified(digest, expected) {
        Ok(_) => true,
        Err(cobble_store::CasError::NotFound) => false,
        Err(e) => return Err(e.into()),
    };
    Ok(java_archive_demand(
        size,
        cached,
        cache_volume,
        runtime_volume,
    ))
}

pub async fn managed_java_space_artifacts(
    required: u32,
    cache_volume: &str,
    runtime_volume: &str,
    control: OperationControl,
) -> Result<Vec<SpaceArtifact>, Error> {
    control.check_cancelled()?;
    let available = tokio::select! {value=cobble_minecraft::resolve_java_override(required,&JavaOverride::Automatic)=>value.is_ok(),
    _=control.cancelled()=>return Err(crate::OperationCancelled.into())};
    if available {
        return Ok(Vec::new());
    }
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err("JAVA_MANAGED_ACQUISITION_UNSUPPORTED_PLATFORM".into());
    }
    let broker = Broker::new(BrokerConfig::default())?;
    let cache = ModDownloads::open()?;
    Ok(vec![
        java_archive_artifact(
            required,
            &broker,
            &cache,
            cache_volume,
            runtime_volume,
            &control,
        )
        .await?,
    ])
}

fn pack_artifacts(
    pack: &cobble_content::Mrpack,
    optional: &[String],
    directory: &Path,
    cache_volume: &str,
    target_volume: &str,
    cache: &ModDownloads,
    control: &OperationControl,
) -> Result<Vec<SpaceArtifact>, Error> {
    let selected: std::collections::BTreeSet<_> = optional.iter().collect();
    if selected.len() != optional.len()
        || selected
            .iter()
            .any(|p| !pack.files.iter().any(|f| f.optional && &f.path == *p))
    {
        return Err("SPACE_PACK_OPTIONAL_INVALID".into());
    }
    let root = match Dir::open_ambient_dir(directory, cap_std::ambient_authority()) {
        Ok(root) => Some(root),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let mut artifacts = Vec::new();
    for file in pack
        .files
        .iter()
        .filter(|f| !f.optional || selected.contains(&f.path))
    {
        control.check_cancelled()?;
        let expected = crate::mrpack::expectation(file)?;
        let installed = if let Some(root) = &root {
            file_matches(
                root,
                &file.path,
                Some(file.file_size),
                Some(&file.sha1),
                control,
            )? == Some(true)
        } else {
            false
        };
        artifacts.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(file.file_size),
            installed_bytes: Some(file.file_size),
            verified_cached: installed || cache.contains_expectation(expected)?,
            already_installed: installed,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        });
    }
    // Overrides stream from ZIP to verified CAS, then into the target. Their
    // uncompressed sizes are known; no guessed compression ratio is needed.
    for item in &pack.overrides {
        control.check_cancelled()?;
        cobble_store::SafeRelativePath::parse(&item.path)?;
        artifacts.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(item.size),
            installed_bytes: Some(item.size),
            verified_cached: false,
            already_installed: false,
            extraction_scratch_bytes: Some(0),
            backup_copy_bytes: 0,
        });
    }
    Ok(artifacts)
}
pub fn mrpack_space_artifacts(
    pack: &cobble_content::Mrpack,
    optional: &[String],
    directory: &Path,
    cache_volume: &str,
    target_volume: &str,
    control: OperationControl,
) -> Result<Vec<SpaceArtifact>, Error> {
    pack_artifacts(
        pack,
        optional,
        directory,
        cache_volume,
        target_volume,
        &ModDownloads::open()?,
        &control,
    )
}

/// Directory is host-owned and may not exist yet. Identifiers group actual
/// cache/game/runtime volumes, so one physical volume is charged only once.
#[allow(clippy::too_many_arguments)]
pub async fn environment_space_artifacts(
    version: &str,
    loader_kind: Option<&str>,
    environment: Option<&EnvironmentPlan>,
    source: DownloadSource,
    directory: &Path,
    java: &JavaOverride,
    cache_volume: &str,
    target_volume: &str,
    runtime_volume: &str,
    control: OperationControl,
) -> Result<ArtifactSpacePlan, Error> {
    control.check_cancelled()?;
    let broker = Broker::new(BrokerConfig::default())?;
    let fetching = async {
        VersionCatalog::fetch_from(&broker, &source)
            .await?
            .fetch_version_from(&broker, version, &source)
            .await
    };
    let metadata = tokio::select! {value=fetching=>value?,_=control.cancelled()=>return Err(crate::OperationCancelled.into())};
    let plan = metadata.download_plan(&PlatformContext::current()?)?;
    let required = metadata.java_version.major_version;
    let root = match Dir::open_ambient_dir(directory, cap_std::ambient_authority()) {
        Ok(dir) => Some(dir),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    let cache = ModDownloads::open()?;
    let mut files = plan.files;
    let index = tokio::select! {value=broker.get(&metadata.asset_index.url,None,32*1024*1024)=>value?,_=control.cancelled()=>return Err(crate::OperationCancelled.into())};
    control.check_cancelled()?;
    if index.metadata.status != 200
        || index.body.len() as u64 != metadata.asset_index.size
        || !format!("{:x}", Sha1::digest(&index.body))
            .eq_ignore_ascii_case(&metadata.asset_index.sha1)
    {
        return Err("SPACE_ASSET_INDEX_INVALID".into());
    }
    files.extend(asset_files(&index.body)?);
    let mut artifacts = Vec::with_capacity(files.len() + 256);
    for file in files {
        control.check_cancelled()?;
        let hash: String = file.sha1.iter().map(|b| format!("{b:02x}")).collect();
        let selected = cobble_content::ModrinthFile {
            url: file.url,
            filename: file.path.as_str().into(),
            size: file.size,
            sha1: hash.clone(),
            sha512: None,
        };
        let installed = match &root {
            Some(root) => {
                file_matches(
                    root,
                    file.path.as_str(),
                    Some(file.size),
                    Some(&hash),
                    &control,
                )? == Some(true)
            }
            None => false,
        };
        artifacts.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: Some(file.size),
            installed_bytes: Some(file.size),
            verified_cached: installed || cache.contains(&selected)?,
            already_installed: installed,
            extraction_scratch_bytes: if file.kind == cobble_minecraft::FileKind::Native
                || file.path.as_str().contains("-natives-")
            {
                None
            } else {
                Some(0)
            },
            backup_copy_bytes: 0,
        });
    }
    if let Some(environment) = environment {
        crate::environment_install::validate_environment_plan(environment)?;
        for graph in [
            environment.modrinth.as_ref(),
            environment.curseforge.as_ref().map(|p| &p.graph),
        ]
        .into_iter()
        .flatten()
        {
            for item in &graph.mods {
                let installed = if let Some(root) = &root {
                    let normal = format!("mods/{}", item.file.filename);
                    let disabled = format!("{normal}.disabled");
                    file_matches(
                        root,
                        &normal,
                        Some(item.file.size),
                        Some(&item.file.sha1),
                        &control,
                    )? == Some(true)
                        || file_matches(
                            root,
                            &disabled,
                            Some(item.file.size),
                            Some(&item.file.sha1),
                            &control,
                        )? == Some(true)
                } else {
                    false
                };
                artifacts.push(SpaceArtifact {
                    cache_volume: cache_volume.into(),
                    target_volume: target_volume.into(),
                    compressed_bytes: Some(item.file.size),
                    installed_bytes: Some(item.file.size),
                    verified_cached: installed || cache.contains(&item.file)?,
                    already_installed: installed,
                    extraction_scratch_bytes: Some(0),
                    backup_copy_bytes: 0,
                });
            }
        }
    }
    // Loader installer outputs/profile sizes are not reliably known before its
    // authoritative installer is inspected; do not invent an upper bound.
    if loader_kind.is_some() {
        artifacts.push(SpaceArtifact {
            cache_volume: cache_volume.into(),
            target_volume: target_volume.into(),
            compressed_bytes: None,
            installed_bytes: None,
            verified_cached: false,
            already_installed: false,
            extraction_scratch_bytes: None,
            backup_copy_bytes: 0,
        });
    }
    let java_available = tokio::select! {value=cobble_minecraft::resolve_java_override(required,java)=>value.is_ok(),_=control.cancelled()=>return Err(crate::OperationCancelled.into())};
    control.check_cancelled()?;
    if !java_available
        && matches!(java, JavaOverride::Automatic)
        && cfg!(all(target_os = "windows", target_arch = "x86_64"))
    {
        artifacts.push(
            java_archive_artifact(
                required,
                &broker,
                &cache,
                cache_volume,
                runtime_volume,
                &control,
            )
            .await?,
        );
    }
    Ok(ArtifactSpacePlan {
        artifacts,
        required_java: required,
        java_available,
    })
}

#[cfg(test)]
#[path = "readiness_space/tests.rs"]
mod tests;

// SPDX-License-Identifier: MIT
//! Fabric's official client profile overlays a separately installed Vanilla base.
//! Independently implemented; XMCL's MIT installer was read as a behavior reference.
use crate::{InstallOutcome, InstallProgress, install_game_controlled};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_minecraft::DownloadSource;
use cobble_net::{Broker, BrokerConfig};
use cobble_store::{
    ArtifactExpectation, ContentStore, ExpectedDigests, IntegrityBasis, SafeRelativePath,
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-fabric.json";
const META: &str = "https://meta.fabricmc.net/v2/versions/loader";
const MAX_JAR: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Deserialize)]
struct LoaderEntry {
    loader: FabricVersion,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FabricState {
    pub mc_version: String,
    pub loader_version: String,
    pub complete: bool,
    pub profile: FabricProfile,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FabricProfile {
    pub id: String,
    pub inherits_from: String,
    pub main_class: String,
    pub arguments: serde_json::Value,
    pub libraries: Vec<FabricLibrary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FabricLibrary {
    name: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

fn identifier(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
    {
        return Err("invalid Minecraft or Fabric version identifier".into());
    }
    Ok(())
}

pub async fn list_fabric_versions(mc_version: &str) -> Result<Vec<FabricVersion>, Error> {
    let source = cobble_minecraft::read_download_settings()?.download_source()?;
    list_fabric_versions_from(mc_version, &source).await
}

pub async fn list_fabric_versions_from(
    mc_version: &str,
    source: &DownloadSource,
) -> Result<Vec<FabricVersion>, Error> {
    identifier(mc_version)?;
    let broker = Broker::new(BrokerConfig::default())?;
    let entries = source
        .fetch_public_validated(
            &broker,
            &format!("{META}/{mc_version}"),
            4 * 1024 * 1024,
            |bytes| -> Result<Vec<LoaderEntry>, Error> { Ok(serde_json::from_slice(bytes)?) },
        )
        .await?;
    Ok(entries.into_iter().map(|entry| entry.loader).collect())
}

/// Install the Vanilla base followed by a pinned Fabric client profile and libraries.
/// Interrupted overlays remain explicitly incomplete and cannot be launched.
pub async fn install_fabric(
    mc_version: &str,
    loader_version: &str,
    target: &Path,
    source: DownloadSource,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<InstallOutcome, Error> {
    install_fabric_controlled(
        mc_version,
        loader_version,
        target,
        source,
        crate::OperationControl::new(on_progress),
    )
    .await
}

pub async fn install_fabric_controlled(
    mc_version: &str,
    loader_version: &str,
    target: &Path,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    if target.join(".cobble-forge.json").exists() {
        return Err("this instance already has Forge or NeoForge".into());
    }
    identifier(mc_version)?;
    identifier(loader_version)?;
    let broker = Broker::new(BrokerConfig {
        total_timeout: std::time::Duration::from_secs(300),
        ..BrokerConfig::default()
    })?;
    let url = format!("{META}/{mc_version}/{loader_version}/profile/json");
    let profile = source
        .fetch_public_validated(
            &broker,
            &url,
            4 * 1024 * 1024,
            |bytes| -> Result<FabricProfile, Error> {
                let profile = serde_json::from_slice(bytes)?;
                validate_profile(&profile, mc_version, loader_version)?;
                Ok(profile)
            },
        )
        .await?;
    control.check_cancelled()?;
    let outcome =
        install_game_controlled(mc_version, target, source.clone(), control.clone()).await?;
    let root = Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
    let mut state = FabricState {
        mc_version: mc_version.into(),
        loader_version: loader_version.into(),
        complete: false,
        profile,
    };
    write_state(&root, &state)?;
    let cache = dirs::cache_dir()
        .ok_or("could not locate the user cache directory")?
        .join("CobbleLauncher");
    std::fs::create_dir_all(cache.join("content"))?;
    std::fs::create_dir_all(cache.join("sha1"))?;
    let content = ContentStore::open(Dir::open_ambient_dir(
        cache.join("content"),
        cap_std::ambient_authority(),
    )?)?;
    let index = Dir::open_ambient_dir(cache.join("sha1"), cap_std::ambient_authority())?;
    let total = state.profile.libraries.len();
    let mut reused = 0;
    on_progress(InstallProgress {
        detail: None,
        phase: "fabric".into(),
        completed: 0,
        total,
        reused,
    });
    for (i, library) in state.profile.libraries.iter().enumerate() {
        control.check_cancelled()?;
        let path = library.path()?;
        let official = format!(
            "{}/{}",
            library.url.trim_end_matches('/'),
            path.as_str().trim_start_matches("libraries/")
        );
        let digest = match &library.sha1 {
            Some(hash) => parse_sha1(hash)?,
            None => {
                // BMCLAPI does not mirror every Maven checksum sidecar.
                // Obtain missing identity from Fabric's public Maven repository.
                let bytes = fetch(&broker, &format!("{official}.sha1"), 256)
                    .await
                    .map_err(|error: Error| -> Error {
                        format!("Fabric library {} checksum: {error}", library.name).into()
                    })?;
                parse_sha1(std::str::from_utf8(&bytes)?.trim())?
            }
        };
        let expected = ArtifactExpectation {
            expected_size: library.size,
            digests: ExpectedDigests {
                sha1: Some(digest),
                ..ExpectedDigests::default()
            },
            max_bytes: MAX_JAR,
            basis: IntegrityBasis::TrustedMetadata,
        };
        let local = if root.exists(path.as_str()) {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut bytes = Vec::new();
            root.open_with(path.as_str(), &options)?
                .take(MAX_JAR + 1)
                .read_to_end(&mut bytes)?;
            // Ingestion validates local bytes against the authorized upstream hash.
            let artifact = content.ingest(bytes.as_slice(), expected)?.artifact;
            let _ = crate::cache::record(&index, &artifact);
            true
        } else {
            let cached = crate::cache::lookup(&content, &index, expected)?;
            let hit = cached.is_some();
            let artifact = match cached {
                Some(artifact) => artifact,
                None => {
                    // Artifact requests stay sequential and retain the longer body timeout.
                    let bytes = source
                        .fetch_public(&broker, &official, MAX_JAR)
                        .await
                        .map_err(|error| -> Error {
                            format!("Fabric library {}: {error}", library.name).into()
                        })?;
                    content.ingest(bytes.as_slice(), expected)?.artifact
                }
            };
            let _ = crate::cache::record(&index, &artifact);
            content.copy_to(&artifact, &root, &path)?;
            hit
        };
        reused += usize::from(local);
        on_progress(InstallProgress {
            detail: None,
            phase: "fabric".into(),
            completed: i + 1,
            total,
            reused,
        });
    }
    control.check_cancelled()?;
    state.complete = true;
    write_state(&root, &state)?;
    on_progress(InstallProgress {
        detail: None,
        phase: "complete".into(),
        completed: 1,
        total: 1,
        reused,
    });
    Ok(outcome)
}

async fn fetch(broker: &Broker, url: &str, limit: u64) -> Result<Vec<u8>, Error> {
    let response = broker.get(url, None, limit).await?;
    if response.metadata.status != 200 {
        return Err(format!("Fabric service returned HTTP {}", response.metadata.status).into());
    }
    Ok(response.body)
}

fn parse_sha1(hash: &str) -> Result<[u8; 20], Error> {
    if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid Fabric library SHA-1".into());
    }
    let mut bytes = [0; 20];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)?;
    }
    Ok(bytes)
}

impl FabricLibrary {
    pub(crate) fn identity(&self) -> &str {
        self.name
            .rsplit_once(':')
            .map_or(self.name.as_str(), |(identity, _)| identity)
    }
    pub(crate) fn path(&self) -> Result<SafeRelativePath, Error> {
        let parts: Vec<_> = self.name.split(':').collect();
        if parts.len() != 3 {
            return Err("unsupported Fabric Maven coordinate".into());
        }
        for part in &parts {
            identifier(part)?;
        }
        if parts[0].split('.').any(|part| part.is_empty()) {
            return Err("invalid Fabric Maven group".into());
        }
        if self.url != "https://maven.fabricmc.net/" {
            return Err("unsupported Fabric library repository".into());
        }
        SafeRelativePath::parse(&format!(
            "libraries/{}/{}/{}/{}-{}.jar",
            parts[0].replace('.', "/"),
            parts[1],
            parts[2],
            parts[1],
            parts[2]
        ))
        .map_err(Into::into)
    }
}

fn validate_profile(profile: &FabricProfile, mc: &str, loader: &str) -> Result<(), Error> {
    if profile.inherits_from != mc
        || profile.id != format!("fabric-loader-{loader}-{mc}")
        || profile.main_class != "net.fabricmc.loader.impl.launch.knot.KnotClient"
    {
        return Err("unsupported or mismatched Fabric client profile".into());
    }
    if profile.libraries.is_empty() {
        return Err("Fabric profile has no libraries".into());
    }
    for library in &profile.libraries {
        library.path()?;
    }
    for kind in ["jvm", "game"] {
        if !profile.arguments[kind].is_array() {
            return Err("unsupported Fabric arguments".into());
        }
    }
    Ok(())
}

fn write_state(root: &Dir, state: &FabricState) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(".cobble-fabric.json.tmp", &options)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(".cobble-fabric.json.tmp", root, MARKER)?;
    Ok(())
}

pub(crate) fn load_overlay(directory: &Path, version: &str) -> Result<Option<FabricState>, Error> {
    let bytes = match std::fs::read(directory.join(MARKER)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let state: FabricState = serde_json::from_slice(&bytes)?;
    if !state.complete || state.mc_version != version {
        return Err(
            "Fabric installation is incomplete or belongs to a different Minecraft version".into(),
        );
    }
    validate_profile(&state.profile, version, &state.loader_version)?;
    Ok(Some(state))
}

pub(crate) fn overlay_classpath(
    state: &FabricState,
    directory: &Path,
) -> Result<Vec<PathBuf>, Error> {
    state
        .profile
        .libraries
        .iter()
        .map(|library| {
            let path = directory.join(library.path()?.as_str());
            if !path.is_file() {
                return Err(format!("missing Fabric library: {}", library.name).into());
            }
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forge_instance_is_rejected_before_metadata_fetch_or_base_writes() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("cobble-fabric-forge-guard-{nonce}"));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join(".cobble-forge.json"), b"owned Forge marker").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let error = runtime
            .block_on(install_fabric(
                "1.21.1",
                "0.16.14",
                &directory,
                DownloadSource::Official,
                Arc::new(|_| {}),
            ))
            .unwrap_err();
        assert!(error.to_string().contains("already has Forge or NeoForge"));
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        assert_eq!(
            std::fs::read(directory.join(".cobble-forge.json")).unwrap(),
            b"owned Forge marker"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn maven_paths_preserve_plus_and_reject_traversal_and_foreign_hosts() {
        let mut library = FabricLibrary {
            name: "net.fabricmc:sponge-mixin:0.1+mixin.0.2".into(),
            url: "https://maven.fabricmc.net/".into(),
            sha1: None,
            size: None,
        };
        assert_eq!(
            library.path().unwrap().as_str(),
            "libraries/net/fabricmc/sponge-mixin/0.1+mixin.0.2/sponge-mixin-0.1+mixin.0.2.jar"
        );
        for name in [
            "../evil:artifact:1",
            "a:b:../evil",
            "a:b:1:classifier",
            "a..b:c:1",
        ] {
            library.name = name.into();
            assert!(library.path().is_err());
        }
        library.name = "a:b:1".into();
        library.url = "https://foreign.invalid/".into();
        assert!(library.path().is_err());
    }
    #[test]
    fn invalid_hash_and_version_are_rejected() {
        assert!(parse_sha1("bad").is_err());
        assert_eq!(
            parse_sha1("0123456789012345678901234567890123456789").unwrap()[0],
            1
        );
        assert!(identifier("../1.21.1").is_err());
    }

    #[test]
    fn persisted_profile_pins_parent_loader_and_refuses_incomplete_install() {
        let profile: FabricProfile = serde_json::from_value(serde_json::json!({
            "id": "fabric-loader-0.16.14-1.21.1", "inheritsFrom": "1.21.1",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "arguments": {"jvm": ["-Dfixture=value with spaces"], "game": []},
            "libraries": [{"name": "net.fabricmc:fabric-loader:0.16.14", "url": "https://maven.fabricmc.net/"}]
        })).unwrap();
        assert!(validate_profile(&profile, "1.21.1", "0.16.14").is_ok());
        assert!(validate_profile(&profile, "1.21", "0.16.14").is_err());
        assert!(validate_profile(&profile, "1.21.1", "0.16.13").is_err());
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("cobble-fabric-{nonce}"));
        std::fs::create_dir(&directory).unwrap();
        let root = Dir::open_ambient_dir(&directory, cap_std::ambient_authority()).unwrap();
        let mut state = FabricState {
            mc_version: "1.21.1".into(),
            loader_version: "0.16.14".into(),
            complete: false,
            profile,
        };
        write_state(&root, &state).unwrap();
        assert!(load_overlay(&directory, "1.21.1").is_err());
        state.complete = true;
        write_state(&root, &state).unwrap();
        let loaded = load_overlay(&directory, "1.21.1").unwrap().unwrap();
        assert_eq!(
            loaded.profile.arguments["jvm"][0],
            "-Dfixture=value with spaces"
        );
        assert!(overlay_classpath(&loaded, &directory).is_err());
        drop(root);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

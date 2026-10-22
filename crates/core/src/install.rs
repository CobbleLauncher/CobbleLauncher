// SPDX-License-Identifier: MIT
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_minecraft::{
    DownloadSource, FileKind, PlannedFile, PlatformContext, VersionCatalog, VersionMetadata,
};
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadManager, DownloadRequest,
};
use cobble_store::{
    ArtifactExpectation, ContentStore, DownloadStaging, ExpectedDigests, IntegrityBasis,
    SafeRelativePath,
};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    path::Path,
    sync::Arc,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-install.json";

/// Resource counts for the current installation phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallProgress {
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub reused: usize,
}

/// Successfully installed resources and the Java requirement for a later launch.
#[derive(Debug, Clone, Serialize)]
pub struct InstallOutcome {
    pub version: String,
    pub java_major: u32,
}

/// Install modern Vanilla game resources using the shared verified content cache.
pub async fn install_game(
    version: &str,
    target: &Path,
    source: DownloadSource,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<InstallOutcome, Error> {
    install_game_controlled(
        version,
        target,
        source,
        crate::OperationControl::new(on_progress),
    )
    .await
}

pub async fn install_game_controlled(
    version: &str,
    target: &Path,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;
    install_resources_controlled(version, target, false, source, control).await
}

#[derive(Serialize, Deserialize)]
struct InstallState {
    version: String,
    client_only: bool,
    complete: bool,
}

/// Install resources, optionally limiting the operation to the client JAR.
pub async fn install_resources(
    version: &str,
    target: &Path,
    client_only: bool,
    source: DownloadSource,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<InstallOutcome, Error> {
    install_resources_controlled(
        version,
        target,
        client_only,
        source,
        crate::OperationControl::new(on_progress),
    )
    .await
}

pub async fn install_resources_controlled(
    version: &str,
    target: &Path,
    client_only: bool,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    on_progress(InstallProgress {
        phase: "metadata".into(),
        completed: 0,
        total: 1,
        reused: 0,
    });
    let broker = Broker::new(BrokerConfig {
        total_timeout: std::time::Duration::from_secs(300),
        ..BrokerConfig::default()
    })?;
    let metadata = VersionCatalog::fetch_from(&broker, &source)
        .await?
        .fetch_version_from(&broker, version, &source)
        .await?;
    let mut plan = metadata.download_plan(&PlatformContext::current()?)?;
    control.check_cancelled()?;
    on_progress(InstallProgress {
        phase: "metadata".into(),
        completed: 1,
        total: 1,
        reused: 0,
    });
    let root = Arc::new(prepare_target(target, version, client_only)?);
    let cache = dirs::cache_dir()
        .ok_or("could not locate the user cache directory")?
        .join("CobbleLauncher");
    std::fs::create_dir_all(cache.join("downloads"))?;
    std::fs::create_dir_all(cache.join("content"))?;
    std::fs::create_dir_all(cache.join("sha1"))?;
    let cache_index = Arc::new(register(&cache.join("sha1"))?);
    let content = Arc::new(ContentStore::open(register(&cache.join("content"))?)?);
    let downloader = DownloadManager::new(ArtifactDownloader::new(
        broker,
        DownloadStaging::open(register(&cache.join("downloads"))?)?,
        ContentStore::open(register(&cache.join("content"))?)?,
    ));
    if client_only {
        plan.files.retain(|file| file.kind == FileKind::Client);
    }
    let index_path = plan
        .files
        .iter()
        .find(|f| f.kind == FileKind::AssetIndex)
        .map(|f| f.path.clone());
    download_files(
        plan.files,
        &downloader,
        &content,
        &root,
        &cache_index,
        &source,
        ("files", &control),
    )
    .await?;
    if let Some(index_path) = index_path {
        let index_root = root.clone();
        let objects = tokio::task::spawn_blocking(move || {
            let file = index_root.open(index_path.as_str())?;
            let index: AssetObjects = serde_json::from_reader(file)?;
            asset_files(index)
        })
        .await??;
        download_files(
            objects,
            &downloader,
            &content,
            &root,
            &cache_index,
            &source,
            ("assets", &control),
        )
        .await?;
    }
    control.check_cancelled()?;
    save_metadata(&root, &metadata, &content)?;
    control.check_cancelled()?;
    write_state(
        &root,
        &InstallState {
            version: version.into(),
            client_only,
            complete: true,
        },
    )?;
    on_progress(InstallProgress {
        phase: "complete".into(),
        completed: 1,
        total: 1,
        reused: 0,
    });
    Ok(InstallOutcome {
        version: version.into(),
        java_major: plan.java_major,
    })
}

fn register(path: &Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
}

pub(crate) fn prepare_target(
    target: &Path,
    version: &str,
    client_only: bool,
) -> Result<Dir, Error> {
    std::fs::create_dir_all(target)?;
    let root = register(target)?;
    if root.entries()?.next().is_some() {
        let state: InstallState = serde_json::from_reader(
            root.open(MARKER)
                .map_err(|_| "choose an empty folder; existing game folders are not overwritten")?,
        )?;
        if state.version != version || state.client_only != client_only {
            return Err(
                "this folder belongs to a different installation; choose an empty folder".into(),
            );
        }
    } else {
        write_state(
            &root,
            &InstallState {
                version: version.into(),
                client_only,
                complete: false,
            },
        )?;
    }
    Ok(root)
}

fn write_state(root: &Dir, state: &InstallState) -> Result<(), Error> {
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let temporary = ".cobble-install.json.tmp";
    let mut file = root.open_with(temporary, &options)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(temporary, root, MARKER)?;
    Ok(())
}

fn save_metadata(
    root: &Dir,
    metadata: &VersionMetadata,
    content: &ContentStore,
) -> Result<(), Error> {
    let relative = SafeRelativePath::parse(&format!("versions/{0}/{0}.json", metadata.id))?;
    if let Ok(mut old) = root.open(relative.as_str()) {
        let mut bytes = Vec::new();
        old.read_to_end(&mut bytes)?;
        if bytes == metadata.source_bytes() {
            return Ok(());
        }
        return Err("existing version JSON differs from the selected version".into());
    }
    let bytes = metadata.source_bytes();
    let result = content.ingest(
        bytes,
        ArtifactExpectation {
            expected_size: Some(bytes.len() as u64),
            digests: ExpectedDigests {
                sha1: Some(Sha1::digest(bytes).into()),
                ..ExpectedDigests::default()
            },
            max_bytes: bytes.len() as u64,
            basis: IntegrityBasis::TrustedMetadata,
        },
    )?;
    content.copy_to(&result.artifact, root, &relative)?;
    Ok(())
}

async fn download_files(
    files: Vec<PlannedFile>,
    downloader: &DownloadManager,
    content: &Arc<ContentStore>,
    root: &Arc<Dir>,
    index: &Arc<Dir>,
    source: &DownloadSource,
    progress: (&str, &crate::OperationControl),
) -> Result<(), Error> {
    let (phase, control) = progress;
    let on_progress = control.progress();
    let total = files.len();
    on_progress(InstallProgress {
        phase: phase.into(),
        completed: 0,
        total,
        reused: 0,
    });
    let mut files = files.into_iter();
    let mut pending = tokio::task::JoinSet::new();
    let mut completed = 0;
    let mut reused = 0;
    let mut failure: Option<Error> = None;
    loop {
        while pending.len() < 4 && !control.is_cancelled() && failure.is_none() {
            let Some(file) = files.next() else { break };
            let (downloader, content, root) = (downloader.clone(), content.clone(), root.clone());
            let index = index.clone();
            let source = source.clone();
            let control = control.clone();
            pending.spawn(async move {
                let path = file.path.as_str().to_owned();
                download_file(file, downloader, content, root, index, source, control)
                    .await
                    .map_err(|error| -> Error {
                        if error.is::<crate::OperationCancelled>() {
                            error
                        } else {
                            format!("{path}: {error}").into()
                        }
                    })
            });
        }
        let Some(result) = pending.join_next().await else {
            break;
        };
        match result {
            Ok(Ok(hit)) => {
                reused += usize::from(hit);
                completed += 1;
            }
            Ok(Err(error)) => {
                if failure.is_none()
                    || (!error.is::<crate::OperationCancelled>()
                        && failure
                            .as_ref()
                            .is_some_and(|previous| previous.is::<crate::OperationCancelled>()))
                {
                    failure = Some(error);
                }
                control.cancel();
            }
            Err(error) => {
                if failure.is_none() {
                    failure = Some(error.into());
                }
                control.cancel();
            }
        }
        on_progress(InstallProgress {
            phase: phase.into(),
            completed,
            total,
            reused,
        });
    }
    downloader.wait_idle().await;
    if let Some(error) = failure {
        return Err(error);
    }
    control.check_cancelled()
}

async fn download_file(
    file: PlannedFile,
    downloader: DownloadManager,
    content: Arc<ContentStore>,
    root: Arc<Dir>,
    index: Arc<Dir>,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<bool, Error> {
    control.check_cancelled()?;
    let path = file.path.clone();
    let check_root = root.clone();
    let (size, sha1) = (file.size, file.sha1);
    if tokio::task::spawn_blocking(move || existing_file_matches(&check_root, &path, size, sha1))
        .await??
    {
        return Ok(true);
    }
    let expected = ArtifactExpectation {
        expected_size: Some(file.size),
        digests: ExpectedDigests {
            sha1: Some(file.sha1),
            ..ExpectedDigests::default()
        },
        max_bytes: file.size,
        basis: IntegrityBasis::TrustedMetadata,
    };
    let lookup_content = content.clone();
    let lookup_index = index.clone();
    let cached = tokio::task::spawn_blocking(move || {
        crate::cache::lookup(&lookup_content, &lookup_index, expected)
    })
    .await??;
    let reused = cached.is_some();
    control.check_cancelled()?;
    let artifact = if let Some(artifact) = cached {
        artifact
    } else {
        let mut downloaded = None;
        let mut last_error = None;
        let candidates = source.candidates(&file.url)?;
        let has_fallback = candidates.len() > 1;
        for (candidate, url) in candidates {
            control.check_cancelled()?;
            let mut request = DownloadRequest::new(
                &url,
                expected,
                DownloadAuthority::same_origin(
                    &candidate.id(),
                    "minecraft-install",
                    "vanilla-v1",
                    None,
                )?,
                None,
            )?;
            if url != file.url {
                request = request.with_mirror_redirects(&file.url)?;
            }
            match downloader.download(request, control.cancellation()).await {
                Ok(result) => {
                    downloaded = Some(result.artifact.clone());
                    break;
                }
                Err(error) => {
                    if control.is_cancelled() {
                        downloader.wait_idle().await;
                        control.check_cancelled()?;
                    }
                    if has_fallback {
                        candidate.report_failure();
                    }
                    last_error = Some(error);
                }
            }
        }
        match downloaded {
            Some(artifact) => artifact,
            None => return Err(last_error.ok_or("No download source available")?.into()),
        }
    };
    control.check_cancelled()?;
    tokio::task::spawn_blocking(move || {
        let _ = crate::cache::record(&index, &artifact);
        content.copy_to(&artifact, &root, &file.path)
    })
    .await??;
    Ok(reused)
}

fn existing_file_matches(
    root: &Dir,
    path: &SafeRelativePath,
    size: u64,
    expected: [u8; 20],
) -> Result<bool, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut file = match root.open_with(path.as_str(), &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != size {
        return Err(format!("existing file differs: {}", path.as_str()).into());
    }
    let mut digest = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if digest.finalize().as_slice() != expected {
        return Err(format!("existing file differs: {}", path.as_str()).into());
    }
    Ok(true)
}

#[derive(Deserialize)]
struct AssetObjects {
    objects: BTreeMap<String, AssetObject>,
    #[serde(default)]
    r#virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}
#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}

fn asset_files(index: AssetObjects) -> Result<Vec<PlannedFile>, Error> {
    if index.r#virtual || index.map_to_resources {
        return Err("legacy virtual assets are not supported yet".into());
    }
    let objects: BTreeMap<_, _> = index
        .objects
        .into_values()
        .map(|o| (o.hash.clone(), o))
        .collect();
    objects
        .into_values()
        .map(|object| {
            let mut sha1 = [0u8; 20];
            if object.hash.len() != 40 || !object.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("invalid asset hash".into());
            }
            for (i, byte) in sha1.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&object.hash[i * 2..i * 2 + 2], 16)?;
            }
            let path = format!("assets/objects/{}/{}", &object.hash[..2], object.hash);
            Ok(PlannedFile {
                path: SafeRelativePath::parse(&path)?,
                url: format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    &object.hash[..2],
                    object.hash
                ),
                size: object.size,
                sha1,
                kind: FileKind::Asset,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_files_report_each_completion_without_network() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let path = std::env::temp_dir()
                    .join(format!("cobble-progress-{}-{nonce}", std::process::id()));
                let root = Arc::new(prepare_target(&path, "demo", false).unwrap());
                for directory in ["content", "index", "downloads"] {
                    root.create_dir(directory).unwrap();
                }
                let content =
                    Arc::new(ContentStore::open(root.open_dir("content").unwrap()).unwrap());
                let downloader = DownloadManager::new(ArtifactDownloader::new(
                    Broker::new(BrokerConfig::default()).unwrap(),
                    DownloadStaging::open(root.open_dir("downloads").unwrap()).unwrap(),
                    ContentStore::open(root.open_dir("content").unwrap()).unwrap(),
                ));
                let files = ["first.jar", "second.jar"]
                    .into_iter()
                    .map(|name| {
                        root.write(name, b"abc").unwrap();
                        PlannedFile {
                            path: SafeRelativePath::parse(name).unwrap(),
                            url: "https://example.invalid/unused".into(),
                            size: 3,
                            sha1: Sha1::digest(b"abc").into(),
                            kind: FileKind::Client,
                        }
                    })
                    .collect();
                let events = Arc::new(std::sync::Mutex::new(Vec::new()));
                let recorded = events.clone();
                let callback: Arc<dyn Fn(InstallProgress) + Send + Sync> = Arc::new(move |event| {
                    recorded.lock().unwrap().push(event);
                });
                let index = Arc::new(root.open_dir("index").unwrap());
                download_files(
                    files,
                    &downloader,
                    &content,
                    &root,
                    &index,
                    &DownloadSource::Official,
                    ("files", &crate::OperationControl::new(callback)),
                )
                .await
                .unwrap();
                {
                    let events = events.lock().unwrap();
                    assert_eq!(events.len(), 3);
                    for (completed, event) in events.iter().enumerate() {
                        assert_eq!(event.phase, "files");
                        assert_eq!(event.completed, completed);
                        assert_eq!(event.total, 2);
                        assert_eq!(event.reused, completed);
                    }
                }
                drop((downloader, content, root, index));
                std::fs::remove_dir_all(path).unwrap();
            });
    }

    #[test]
    fn asset_index_deduplicates_content_and_uses_mojang_object_paths() {
        let index = serde_json::from_slice(
            br#"{"objects":{
            "a":{"hash":"0123456789012345678901234567890123456789","size":3},
            "b":{"hash":"0123456789012345678901234567890123456789","size":3}
        }}"#,
        )
        .unwrap();
        let files = asset_files(index).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files[0].path.as_str(),
            "assets/objects/01/0123456789012345678901234567890123456789"
        );
        assert!(
            files[0]
                .url
                .starts_with("https://resources.download.minecraft.net/01/")
        );
        assert_eq!(files[0].kind, FileKind::Asset);
        assert!(
            asset_files(AssetObjects {
                objects: BTreeMap::new(),
                r#virtual: true,
                map_to_resources: false
            })
            .is_err()
        );
    }

    #[test]
    fn installation_resumes_only_its_own_folder_and_checks_existing_files() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("cobble-install-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        let root = prepare_target(&path, "demo", false).unwrap();
        assert!(prepare_target(&path, "demo", false).is_ok());
        assert!(prepare_target(&path, "other", false).is_err());
        let target = SafeRelativePath::parse("client.jar").unwrap();
        assert!(!existing_file_matches(&root, &target, 3, Sha1::digest(b"abc").into()).unwrap());
        root.write("client.jar", b"abc").unwrap();
        assert!(existing_file_matches(&root, &target, 3, Sha1::digest(b"abc").into()).unwrap());
        assert!(existing_file_matches(&root, &target, 3, [0; 20]).is_err());
        assert!(existing_file_matches(&root, &target, 4, [0; 20]).is_err());
        let unrelated = path.join("unrelated");
        std::fs::create_dir(&unrelated).unwrap();
        std::fs::write(unrelated.join("world.dat"), b"keep").unwrap();
        assert!(prepare_target(&unrelated, "demo", false).is_err());
        assert_eq!(std::fs::read(unrelated.join("world.dat")).unwrap(), b"keep");
        drop(root);
        std::fs::remove_dir_all(&path).unwrap();
    }
}

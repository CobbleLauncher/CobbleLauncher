// SPDX-License-Identifier: MIT
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_minecraft::{
    DownloadSource, FileKind, PlannedFile, PlatformContext, VersionCatalog, VersionMetadata,
};
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadManager, DownloadRequest,
    TransferCancellation,
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

#[derive(Serialize, Deserialize)]
struct InstallState {
    version: String,
    client_only: bool,
    complete: bool,
}

pub fn info(version: &str) -> Result<(), Error> {
    let runtime = runtime()?;
    runtime.block_on(async {
        let broker = Broker::new(BrokerConfig::default())?;
        let source = cobble_minecraft::read_download_settings()?.download_source()?;
        let metadata = VersionCatalog::fetch_from(&broker, &source)
            .await?
            .fetch_version_from(&broker, version, &source)
            .await?;
        let plan = metadata.download_plan(&PlatformContext::current()?)?;
        println!("Minecraft {}", plan.version_id);
        println!("Java: {}", plan.java_major);
        println!("Main class: {}", plan.main_class);
        println!(
            "Client/libraries/index/logging: {} files, {:.1} MiB",
            plan.files.len(),
            plan.files.iter().map(|f| f.size).sum::<u64>() as f64 / 1_048_576.0
        );
        println!("Assets index: {}", plan.asset_index.id);
        Ok(())
    })
}

pub fn run(
    version: &str,
    target: &Path,
    client_only: bool,
    source: DownloadSource,
) -> Result<(), Error> {
    let runtime = runtime()?;
    let result = runtime.block_on(install(version, target, client_only, source));
    // Blocking transfer/copy workers must finish before process exit.
    drop(runtime);
    result
}

fn runtime() -> io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

pub fn show_java(major: u32) -> Result<(), Error> {
    let java = runtime()?.block_on(cobble_minecraft::find_java(major))?;
    println!("Java {} · {} · {}", java.version, java.vendor, java.arch);
    println!("{}", java.executable.display());
    Ok(())
}

async fn install(
    version: &str,
    target: &Path,
    client_only: bool,
    source: DownloadSource,
) -> Result<(), Error> {
    let broker = Broker::new(BrokerConfig {
        total_timeout: std::time::Duration::from_secs(300),
        ..BrokerConfig::default()
    })?;
    let metadata = VersionCatalog::fetch_from(&broker, &source)
        .await?
        .fetch_version_from(&broker, version, &source)
        .await?;
    let mut plan = metadata.download_plan(&PlatformContext::current()?)?;
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
    println!("Minecraft {} · Java {}", plan.version_id, plan.java_major);
    println!("Download source: {}", source.attribution());
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
        download_files(objects, &downloader, &content, &root, &cache_index, &source).await?;
    }
    save_metadata(&root, &metadata, &content)?;
    write_state(
        &root,
        &InstallState {
            version: version.into(),
            client_only,
            complete: true,
        },
    )?;
    if client_only {
        println!(
            "Client downloaded to {} (libraries, assets and Java not installed)",
            target.display()
        );
    } else {
        println!(
            "Game files downloaded to {}. Java {} and sign-in are required to launch.",
            target.display(),
            plan.java_major
        );
        if let Ok(java) = cobble_minecraft::find_java(plan.java_major).await {
            println!("Java {} found: {}", java.version, java.executable.display());
        }
    }
    Ok(())
}

fn register(path: &Path) -> io::Result<Dir> {
    Dir::open_ambient_dir(path, cap_std::ambient_authority())
}

fn prepare_target(target: &Path, version: &str, client_only: bool) -> Result<Dir, Error> {
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
) -> Result<(), Error> {
    let total = files.len();
    let report_every = if total > 256 { 256 } else { 32 };
    let mut files = files.into_iter();
    let mut pending = tokio::task::JoinSet::new();
    let mut completed = 0;
    let mut reused = 0;
    loop {
        while pending.len() < 4 {
            let Some(file) = files.next() else { break };
            let (downloader, content, root) = (downloader.clone(), content.clone(), root.clone());
            let index = index.clone();
            let source = source.clone();
            pending.spawn(async move {
                let path = file.path.as_str().to_owned();
                download_file(file, downloader, content, root, index, source)
                    .await
                    .map_err(|error| -> Error { format!("{path}: {error}").into() })
            });
        }
        let Some(result) = pending.join_next().await else {
            break;
        };
        reused += usize::from(result??);
        completed += 1;
        if completed % report_every == 0 || completed == total {
            eprintln!("Files ready: {completed}/{total} (reused: {reused})");
        }
    }
    Ok(())
}

async fn download_file(
    file: PlannedFile,
    downloader: DownloadManager,
    content: Arc<ContentStore>,
    root: Arc<Dir>,
    index: Arc<Dir>,
    source: DownloadSource,
) -> Result<bool, Error> {
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
    let artifact = if let Some(artifact) = cached {
        artifact
    } else {
        let url = source.resolve(&file.url)?;
        let mut request = DownloadRequest::new(
            &url,
            expected,
            DownloadAuthority::same_origin(&source.id(), "minecraft-install", "vanilla-v1", None)?,
            None,
        )?;
        if url != file.url {
            request = request.with_mirror_redirects(&file.url)?;
        }
        let result = downloader
            .download(request, TransferCancellation::default())
            .await?;
        result.artifact.clone()
    };
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

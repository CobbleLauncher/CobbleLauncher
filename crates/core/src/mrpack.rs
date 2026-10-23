// SPDX-License-Identifier: MIT
//! First installation of a pinned local MRPACK. Caller holds the instance lease.
//! Files are copied from verified CAS objects; completed packs are never updated.
use crate::InstallProgress;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{Mrpack, MrpackFile, read_mrpack_file};
use cobble_minecraft::DownloadSource;
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadManager, DownloadRequest,
};
use cobble_store::{
    ArtifactExpectation, ContentStore, DownloadStaging, ExpectedDigests, IntegrityBasis,
    SafeRelativePath, VerifiedArtifact,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-pack.json";

#[derive(Debug, Clone, Serialize)]
pub struct MrpackInstallOutcome {
    pub name: String,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct State {
    name: String,
    version_id: String,
    archive_sha256: String,
    selected_optional: BTreeSet<String>,
    complete: bool,
}

/// Install only into a new folder, or resume this exact incomplete archive and
/// optional selection. The selected archive must remain unchanged while running.
/// No game process is started. This is not a pack update or multi-file transaction.
pub async fn install_mrpack(
    archive: &Path,
    target: &Path,
    source: DownloadSource,
    optional_files: &[String],
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<MrpackInstallOutcome, Error> {
    install_mrpack_controlled(
        archive,
        target,
        source,
        optional_files,
        crate::OperationControl::new(on_progress),
    )
    .await
}

pub async fn install_mrpack_controlled(
    archive: &Path,
    target: &Path,
    source: DownloadSource,
    optional_files: &[String],
    control: crate::OperationControl,
) -> Result<MrpackInstallOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    let mut input = std::fs::File::open(archive)?;
    if !input.metadata()?.is_file() || input.metadata()?.len() > 16 * 1024 * 1024 * 1024 {
        return Err("pack archive must be a regular file no larger than 16 GiB".into());
    }
    let archive_sha256 = hash_reader_controlled(&mut input, &control)?;
    input.seek(SeekFrom::Start(0))?;
    let pack = read_mrpack_file(input.try_clone()?)?;
    let selected: BTreeSet<String> = optional_files.iter().cloned().collect();
    for path in &selected {
        if !pack
            .files
            .iter()
            .any(|file| file.optional && &file.path == path)
        {
            return Err(format!("unknown optional pack file: {path}").into());
        }
    }
    let mut state = State {
        name: pack.name.clone(),
        version_id: pack.version_id.clone(),
        archive_sha256,
        selected_optional: selected,
        complete: false,
    };
    let root = prepare(target, &pack, &state)?;
    write_state(&root, &state)?;
    let base_progress = on_progress.clone();
    let base_progress: Arc<dyn Fn(InstallProgress) + Send + Sync> = Arc::new(move |mut event| {
        event.phase = format!("pack-base-{}", event.phase);
        base_progress(event);
    });
    let base_control = control.with_progress(base_progress);
    match (pack.loader_kind.as_deref(), pack.loader_version.as_deref()) {
        (Some("fabric"), Some(loader)) => {
            crate::install_fabric_controlled(
                &pack.minecraft_version,
                loader,
                target,
                source,
                base_control,
            )
            .await?;
        }
        (Some(kind @ ("forge" | "neoforge")), Some(loader)) => {
            let kind = if kind == "forge" {
                crate::ForgeKind::Forge
            } else {
                crate::ForgeKind::NeoForge
            };
            crate::install_forge_controlled(
                &pack.minecraft_version,
                loader,
                kind,
                target,
                source,
                base_control,
            )
            .await?;
        }
        (None, None) => {
            crate::install_game_controlled(&pack.minecraft_version, target, source, base_control)
                .await?;
        }
        _ => return Err("unsupported or incomplete pack loader selection".into()),
    }
    let cache = dirs::cache_dir()
        .ok_or("could not locate user cache")?
        .join("CobbleLauncher");
    for name in ["downloads", "content", "sha1"] {
        std::fs::create_dir_all(cache.join(name))?;
    }
    let open = |name: &str| Dir::open_ambient_dir(cache.join(name), cap_std::ambient_authority());
    let content = ContentStore::open(open("content")?)?;
    let index = open("sha1")?;
    let downloader = DownloadManager::new(ArtifactDownloader::new(
        Broker::new(BrokerConfig {
            total_timeout: std::time::Duration::from_secs(300),
            ..BrokerConfig::default()
        })?,
        DownloadStaging::open(open("downloads")?)?,
        ContentStore::open(open("content")?)?,
    ));
    let files: Vec<_> = pack
        .files
        .iter()
        .filter(|file| !file.optional || state.selected_optional.contains(&file.path))
        .collect();
    let mut final_files = BTreeMap::new();
    on_progress(progress("pack-download", 0, files.len()));
    for (number, file) in files.iter().enumerate() {
        control.check_cancelled()?;
        let expected = expectation(file)?;
        let artifact = match crate::cache::lookup(&content, &index, expected)? {
            Some(artifact) => artifact,
            None => download(file, expected, &downloader, &control).await?,
        };
        let _ = crate::cache::record(&index, &artifact);
        final_files.insert(file.path.clone(), artifact);
        on_progress(progress("pack-download", number + 1, files.len()));
    }
    // Resolve layers before publication: overridden downloads never briefly
    // replace a user's file, and a retry checks only the final selected bytes.
    input.seek(SeekFrom::Start(0))?;
    let mut zip = zip::ZipArchive::new(input)?;
    apply_overrides(&mut zip, &pack, &content, &mut final_files)?;
    control.check_cancelled()?;
    publish_controlled(&root, &content, &final_files, &control)?;
    control.check_cancelled()?;
    state.complete = true;
    write_state(&root, &state)?;
    on_progress(progress("complete", final_files.len(), final_files.len()));
    Ok(MrpackInstallOutcome {
        name: pack.name,
        version_id: pack.version_id,
        minecraft_version: pack.minecraft_version,
        loader_kind: pack.loader_kind,
        loader_version: pack.loader_version,
    })
}

fn apply_overrides<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    pack: &Mrpack,
    content: &ContentStore,
    final_files: &mut BTreeMap<String, VerifiedArtifact>,
) -> Result<(), Error> {
    apply_pack_overrides(zip, &pack.overrides, content, final_files)
}

pub(super) fn apply_pack_overrides<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    overrides: &[cobble_content::PackOverride],
    content: &ContentStore,
    final_files: &mut BTreeMap<String, VerifiedArtifact>,
) -> Result<(), Error> {
    for layer in [0, 1] {
        for entry in overrides.iter().filter(|entry| entry.layer == layer) {
            let mut file = zip.by_index(entry.entry_index)?;
            if file.unix_mode().is_some_and(|mode| {
                let kind = mode & 0o170000;
                kind != 0 && kind != 0o100000
            }) || file.is_dir()
                || file.size() != entry.size
            {
                return Err("pack override changed or is a symlink".into());
            }
            let artifact = content
                .ingest(
                    &mut file,
                    ArtifactExpectation {
                        expected_size: Some(entry.size),
                        max_bytes: entry.size,
                        basis: IntegrityBasis::UserConfirmedLocal,
                        digests: ExpectedDigests::default(),
                    },
                )?
                .artifact;
            final_files.insert(entry.path.clone(), artifact);
        }
    }
    Ok(())
}

pub(crate) fn ensure_pack_complete(directory: &Path) -> Result<(), Error> {
    crate::curseforge_pack::ensure_complete(directory)?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let marker = match root.open_with(MARKER, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let state: State = serde_json::from_reader(marker.take(1024 * 1024))?;
    if !state.complete {
        return Err("pack installation is incomplete; retry before launching".into());
    }
    Ok(())
}

fn prepare(target: &Path, pack: &Mrpack, state: &State) -> Result<Dir, Error> {
    if target.exists() {
        let root = Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
        if root.entries()?.next().is_some() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let old: State = serde_json::from_reader(
                root.open_with(MARKER, &options)
                    .map_err(|_| "pack installation requires an empty folder")?
                    .take(1024 * 1024),
            )?;
            if old.complete {
                return Err(
                    "pack is already installed; updates and reimport are not supported".into(),
                );
            }
            if old.archive_sha256 != state.archive_sha256
                || old.selected_optional != state.selected_optional
            {
                return Err(
                    "retry requires the exact same pack archive and optional selection".into(),
                );
            }
        }
    }
    crate::install::prepare_target(target, &pack.minecraft_version, false)
}

async fn download(
    file: &MrpackFile,
    expected: ArtifactExpectation,
    downloader: &DownloadManager,
    control: &crate::OperationControl,
) -> Result<VerifiedArtifact, Error> {
    let mut last = None;
    for url in &file.downloads {
        control.check_cancelled()?;
        let request = DownloadRequest::new(
            url,
            expected,
            DownloadAuthority::same_origin("mrpack", "pack-install", "selected-archive-v1", None)?,
            None,
        )?
        .with_modpack_redirects()?;
        let result = downloader.download(request, control.cancellation()).await;
        downloader.wait_idle().await;
        control.check_cancelled()?;
        match result {
            Ok(outcome) => return Ok(outcome.artifact.clone()),
            Err(error) => last = Some(error),
        }
    }
    Err(format!(
        "pack download failed for {}: {}",
        file.path,
        last.map(|error| error.to_string())
            .unwrap_or_else(|| "no download URLs".into())
    )
    .into())
}

fn expectation(file: &MrpackFile) -> Result<ArtifactExpectation, Error> {
    Ok(ArtifactExpectation {
        expected_size: Some(file.file_size),
        max_bytes: file.file_size,
        basis: IntegrityBasis::TrustedMetadata,
        digests: ExpectedDigests {
            sha1: Some(digest(&file.sha1)?),
            sha512: Some(digest(&file.sha512)?),
            ..ExpectedDigests::default()
        },
    })
}

fn digest<const N: usize>(value: &str) -> Result<[u8; N], Error> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid pack hash".into());
    }
    let mut bytes = [0; N];
    for (number, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[number * 2..number * 2 + 2], 16)?;
    }
    Ok(bytes)
}

fn hash_reader(input: &mut impl Read) -> Result<String, Error> {
    hash_reader_controlled(input, &crate::OperationControl::new(Arc::new(|_| {})))
}

pub(super) fn hash_reader_controlled(
    input: &mut impl Read,
    control: &crate::OperationControl,
) -> Result<String, Error> {
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        control.check_cancelled()?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
fn publish(
    root: &Dir,
    content: &ContentStore,
    files: &BTreeMap<String, VerifiedArtifact>,
    on_progress: &Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<(), Error> {
    publish_controlled(
        root,
        content,
        files,
        &crate::OperationControl::new(on_progress.clone()),
    )
}

pub(super) fn publish_controlled(
    root: &Dir,
    content: &ContentStore,
    files: &BTreeMap<String, VerifiedArtifact>,
    control: &crate::OperationControl,
) -> Result<(), Error> {
    let on_progress = control.progress();
    // Preflight every destination before publishing any payload. Existing files
    // are reused only on exact digest match; local edits always block retry.
    for (path, artifact) in files {
        control.check_cancelled()?;
        existing(root, path, artifact)?;
    }
    on_progress(progress("pack-install", 0, files.len()));
    for (number, (path, artifact)) in files.iter().enumerate() {
        control.check_cancelled()?;
        if !existing(root, path, artifact)? {
            content.copy_to(artifact, root, &SafeRelativePath::parse(path)?)?;
        }
        on_progress(progress("pack-install", number + 1, files.len()));
    }
    Ok(())
}

fn existing(root: &Dir, path: &str, artifact: &VerifiedArtifact) -> Result<bool, Error> {
    let safe = SafeRelativePath::parse(path)?;
    let mut parent = root.try_clone()?;
    let mut parts = safe.as_str().split('/').peekable();
    while let Some(component) = parts.next() {
        if parts.peek().is_some() {
            parent = match parent.open_dir_nofollow(component) {
                Ok(dir) => dir,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.into()),
            };
        } else {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut file = match parent.open_with(component, &options) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.into()),
            };
            if !file.metadata()?.is_file()
                || file.metadata()?.len() != artifact.size_bytes()
                || hash_reader(&mut file)? != artifact.sha256_hex()
            {
                return Err(format!(
                    "pack destination differs; preserve it and choose a new folder: {path}"
                )
                .into());
            }
        }
    }
    Ok(true)
}

fn write_state(root: &Dir, state: &State) -> Result<(), Error> {
    let temporary = ".cobble-pack.json.tmp";
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(temporary, &options)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(temporary, root, MARKER)?;
    Ok(())
}

fn progress(phase: &str, completed: usize, total: usize) -> InstallProgress {
    InstallProgress {
        detail: None,
        phase: phase.into(),
        completed,
        total,
        reused: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cobble-pack-{stamp}-{}",
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("content")).unwrap();
            std::fs::create_dir_all(path.join("game")).unwrap();
            Self(path)
        }
        fn root(&self) -> Dir {
            Dir::open_ambient_dir(self.0.join("game"), cap_std::ambient_authority()).unwrap()
        }
        fn content(&self) -> ContentStore {
            ContentStore::open(
                Dir::open_ambient_dir(self.0.join("content"), cap_std::ambient_authority())
                    .unwrap(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn artifact(content: &ContentStore, bytes: &[u8]) -> VerifiedArtifact {
        content
            .ingest(
                bytes,
                ArtifactExpectation {
                    expected_size: Some(bytes.len() as u64),
                    max_bytes: bytes.len() as u64,
                    basis: IntegrityBasis::UserConfirmedLocal,
                    digests: ExpectedDigests::default(),
                },
            )
            .unwrap()
            .artifact
    }
    fn pack() -> Mrpack {
        Mrpack {
            name: "Synthetic".into(),
            version_id: "one".into(),
            summary: None,
            minecraft_version: "1.21.1".into(),
            loader_kind: None,
            loader_version: None,
            files: vec![],
            overrides: vec![],
        }
    }
    fn state() -> State {
        State {
            name: "Synthetic".into(),
            version_id: "one".into(),
            archive_sha256: "a".repeat(64),
            selected_optional: BTreeSet::new(),
            complete: false,
        }
    }

    #[test]
    fn failed_pack_resume_requires_exact_archive_options_and_preserves_completed_pack() {
        let fixture = Fixture::new();
        let target = fixture.0.join("game");
        let pack = pack();
        let mut state = state();
        let root = prepare(&target, &pack, &state).unwrap();
        write_state(&root, &state).unwrap();
        assert!(prepare(&target, &pack, &state).is_ok());
        state.selected_optional.insert("mods/optional.jar".into());
        assert!(prepare(&target, &pack, &state).is_err());
        state.selected_optional.clear();
        state.archive_sha256 = "b".repeat(64);
        assert!(prepare(&target, &pack, &state).is_err());
        state.archive_sha256 = "a".repeat(64);
        state.complete = true;
        write_state(&root, &state).unwrap();
        assert!(prepare(&target, &pack, &state).is_err());
    }

    #[test]
    fn publication_reuses_exact_bytes_but_preflights_all_local_edits() {
        let fixture = Fixture::new();
        let content = fixture.content();
        let root = fixture.root();
        let files = BTreeMap::from([
            ("config/a.txt".into(), artifact(&content, b"a")),
            ("config/z.txt".into(), artifact(&content, b"z")),
        ]);
        let callback: Arc<dyn Fn(InstallProgress) + Send + Sync> = Arc::new(|_| {});
        root.create_dir("config").unwrap();
        root.write("config/z.txt", b"user edit").unwrap();
        assert!(publish(&root, &content, &files, &callback).is_err());
        assert!(!fixture.0.join("game/config/a.txt").exists());
        root.remove_file("config/z.txt").unwrap();
        publish(&root, &content, &files, &callback).unwrap();
        publish(&root, &content, &files, &callback).unwrap();
        assert_eq!(root.read("config/a.txt").unwrap(), b"a");
        root.write("config/a.txt", b"changed").unwrap();
        assert!(publish(&root, &content, &files, &callback).is_err());
        assert_eq!(root.read("config/a.txt").unwrap(), b"changed");
    }

    #[test]
    fn client_overrides_win_and_server_overrides_are_not_installed() {
        let fixture = Fixture::new();
        let content = fixture.content();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in [
            ("overrides/config/test.txt", b"common".as_slice()),
            ("client-overrides/config/test.txt", b"client"),
            ("server-overrides/config/server.txt", b"server"),
        ] {
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        let mut zip = zip::ZipArchive::new(writer.finish().unwrap()).unwrap();
        let mut pack = pack();
        pack.overrides = vec![
            cobble_content::PackOverride {
                entry_index: 0,
                path: "config/test.txt".into(),
                layer: 0,
                size: 6,
            },
            cobble_content::PackOverride {
                entry_index: 1,
                path: "config/test.txt".into(),
                layer: 1,
                size: 6,
            },
            cobble_content::PackOverride {
                entry_index: 2,
                path: "config/server.txt".into(),
                layer: 2,
                size: 6,
            },
        ];
        let mut files =
            BTreeMap::from([("config/test.txt".into(), artifact(&content, b"download"))]);
        apply_overrides(&mut zip, &pack, &content, &mut files).unwrap();
        assert_eq!(files.len(), 1);
        let callback: Arc<dyn Fn(InstallProgress) + Send + Sync> = Arc::new(|_| {});
        publish(&fixture.root(), &content, &files, &callback).unwrap();
        assert_eq!(fixture.root().read("config/test.txt").unwrap(), b"client");
        assert!(!fixture.0.join("game/config/server.txt").exists());
    }

    #[test]
    fn corrupted_override_crc_and_wrong_download_digest_are_rejected() {
        let fixture = Fixture::new();
        let content = fixture.content();
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "overrides/config/test.txt",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"sample").unwrap();
        let mut bytes = writer.finish().unwrap().into_inner();
        let offset = zip::ZipArchive::new(Cursor::new(&bytes))
            .unwrap()
            .by_index(0)
            .unwrap()
            .data_start()
            .unwrap() as usize;
        bytes[offset] ^= 1;
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut pack = pack();
        pack.overrides.push(cobble_content::PackOverride {
            entry_index: 0,
            path: "config/test.txt".into(),
            layer: 0,
            size: 6,
        });
        assert!(apply_overrides(&mut archive, &pack, &content, &mut BTreeMap::new()).is_err());
        let file = MrpackFile {
            path: "mods/sample.jar".into(),
            downloads: vec![],
            file_size: 6,
            sha1: "0".repeat(40),
            sha512: "0".repeat(128),
            optional: false,
        };
        assert!(
            content
                .ingest(b"sample".as_slice(), expectation(&file).unwrap())
                .is_err()
        );
        assert!(!fixture.0.join("game/config/test.txt").exists());
    }
}

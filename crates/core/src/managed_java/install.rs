// SPDX-License-Identifier: MIT
use super::{
    Error,
    metadata::{self, Package},
};
use crate::{InstallProgress, OperationControl};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_minecraft::JavaRuntime;
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadManager, DownloadRequest,
};
use cobble_store::{
    ArtifactExpectation, ContentStore, DownloadStaging, ExpectedDigests, IntegrityBasis,
    SafeRelativePath,
};
use std::{
    io::{Read, Seek, Write},
    path::Path,
    sync::Arc,
    time::Duration,
};
fn phase(control: &OperationControl, name: &str, done: usize, total: usize) {
    (control.progress())(InstallProgress {
        detail: None,
        phase: name.into(),
        completed: done,
        total,
        reused: 0,
    });
}
fn open(path: &Path) -> Result<Dir, Error> {
    Ok(Dir::open_ambient_dir(path, cap_std::ambient_authority())?)
}
pub(super) async fn install_at(
    required: u32,
    path: &Path,
    cache: &Path,
    control: OperationControl,
) -> Result<JavaRuntime, Error> {
    control.check_cancelled()?;
    let platform = super::target::Target::host()?;
    std::fs::create_dir_all(path)?;
    let root = open(path)?;
    let lock = root
        .open_with(
            format!(".install-{required}-{}.lock", platform.key()),
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .follow(FollowSymlinks::No),
        )?
        .into_std();
    loop {
        control.check_cancelled()?;
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                tokio::select! { _ = control.cancelled() => { control.check_cancelled()?; }, _ = tokio::time::sleep(Duration::from_millis(100)) => {} }
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    // Another instance may have completed this shared runtime while we waited.
    for entry in root.entries()?.take(128) {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_str().is_some_and(|name| {
            name.starts_with(&format!("temurin-{required}-{}-", platform.key()))
        }) && root.open_dir_nofollow(&name).is_ok()
            && let Ok(runtime) =
                cobble_minecraft::probe_java(&path.join(&name).join(platform.executable)).await
            && runtime.major == required
            && platform.matches_arch(&runtime.arch)
        {
            return Ok(runtime);
        }
    }
    phase(&control, "java-metadata", 0, 1);
    let broker = Broker::new(BrokerConfig {
        total_timeout: Duration::from_secs(300),
        ..Default::default()
    })?;
    let package = selection(&root, required, &broker, &control).await?;
    phase(&control, "java-metadata", 1, 1);
    for folder in ["downloads", "content"] {
        std::fs::create_dir_all(cache.join(folder))?;
    }
    let content = Arc::new(ContentStore::open(open(&cache.join("content"))?)?);
    let manager = DownloadManager::new(ArtifactDownloader::new(
        broker,
        DownloadStaging::open(open(&cache.join("downloads"))?)?,
        ContentStore::open(open(&cache.join("content"))?)?,
    ));
    let expectation = ArtifactExpectation {
        expected_size: Some(package.size),
        max_bytes: package.size,
        digests: ExpectedDigests {
            sha256: Some(package.sha256),
            ..Default::default()
        },
        basis: IntegrityBasis::TrustedMetadata,
    };
    let lookup = content.clone();
    let checksum = package.sha256;
    let existing =
        tokio::task::spawn_blocking(move || lookup.open_verified(checksum, expectation)).await?;
    let artifact = match existing {
        Ok(artifact) => artifact,
        Err(cobble_store::CasError::NotFound) => {
            let request = DownloadRequest::new(
                &package.url,
                expectation,
                DownloadAuthority::same_origin(
                    "adoptium",
                    "managed-java",
                    "temurin-ga-archive-v1",
                    None,
                )?,
                None,
            )?
            .with_adoptium_redirects()?;
            let downloading = manager.download(request, control.cancellation());
            tokio::pin!(downloading);
            let mut tick = tokio::time::interval(Duration::from_millis(200));
            let result = loop {
                tokio::select! { result = &mut downloading => break result, _ = tick.tick() => {
                    let progress = manager.metrics();
                    let completed = progress.bytes_received.min(package.size);
                    (control.progress())(InstallProgress { detail: Some(crate::DownloadProgressDetail { bytes_completed: completed, bytes_total: package.size, bytes_received: progress.bytes_received, bytes_per_second: 0, active_downloads: progress.active_downloads, items: progress.items }), phase: "java-download".into(), completed: completed as usize, total: package.size as usize, reused: 0 });
                } }
            };
            manager.wait_idle().await;
            control.check_cancelled()?;
            result?.artifact.clone()
        }
        Err(error) => return Err(error.into()),
    };
    phase(
        &control,
        "java-download",
        package.size as usize,
        package.size as usize,
    );
    control.check_cancelled()?;
    let stage = Stage::new(root)?;
    let staging = stage.dir()?;
    let copied = content.clone();
    let staging = tokio::task::spawn_blocking(move || {
        copied.copy_to(
            &artifact,
            &staging,
            &SafeRelativePath::parse("runtime.zip")?,
        )?;
        Ok::<_, Error>(staging)
    })
    .await??;
    phase(&control, "java-install", 0, 1);
    let extracting = control.clone();
    let top = tokio::task::spawn_blocking(move || extract(&staging, extracting)).await??;
    control.check_cancelled()?;
    phase(&control, "java-install", 1, 1);
    phase(&control, "java-verify", 0, 1);
    let executable = path.join(&stage.name).join(&top).join(platform.executable);
    let runtime = cobble_minecraft::probe_java(&executable).await?;
    if runtime.major != required || !platform.matches_arch(&runtime.arch) {
        return Err("JAVA_RUNTIME_PROBE_MISMATCH".into());
    }
    let staged = stage.dir()?.open_dir_nofollow(&top)?;
    let mut marker = staged.open_with(
        ".cobble-runtime.json",
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No),
    )?;
    serde_json::to_writer_pretty(
        &mut marker,
        &serde_json::json!({"schema_version":1,"provider":"adoptium","metadata_url":metadata::metadata_url(required)?,"package":package,"license":"GPL-2.0-only WITH Classpath-exception-2.0"}),
    )?;
    marker.sync_all()?;
    drop(marker);
    // Windows directory capabilities deliberately deny renaming while open.
    drop(staged);
    control.check_cancelled()?;
    let suffix = package
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let base = format!("temurin-{required}-{}-{}", platform.key(), &suffix[..16]);
    // Never replace files which may still belong to a running JVM. A damaged
    // managed publication gets a separate verified replacement, bounded here.
    let target = publication_name(&stage.root, &base)?;
    stage
        .root
        .rename(format!("{}/{top}", stage.name), &stage.root, &target)?;
    phase(&control, "java-verify", 1, 1);
    Ok(JavaRuntime {
        executable: std::fs::canonicalize(path.join(target).join(platform.executable))?,
        ..runtime
    })
}
fn publication_name(root: &Dir, base: &str) -> Result<String, Error> {
    (0..32)
        .map(|n| {
            if n == 0 {
                base.to_owned()
            } else {
                format!("{base}-r{n}")
            }
        })
        .find_map(|name| match root.try_exists(&name) {
            Ok(false) => Some(Ok(name)),
            Ok(true) => None,
            Err(error) => Some(Err(error)),
        })
        .transpose()?
        .ok_or_else(|| "JAVA_MANAGED_REPLACEMENT_LIMIT".into())
}
async fn selection(
    root: &Dir,
    required: u32,
    broker: &Broker,
    control: &OperationControl,
) -> Result<Package, Error> {
    let name = format!(
        ".selection-{required}-{}.json",
        super::target::Target::host()?.key()
    );
    if let Some(package) = cached_selection(root, required, &name)? {
        return Ok(package);
    }
    let bytes = {
        let url = metadata::metadata_url(required)?;
        let response = tokio::select! { result = broker.get(&url, None, 1024*1024) => result?, _ = control.cancelled() => { control.check_cancelled()?; return Err("JAVA_CANCELLED".into()); } };
        if response.metadata.status != 200
            || response.metadata.final_origin != "https://api.adoptium.net"
        {
            return Err("JAVA_METADATA_REQUEST_FAILED".into());
        }
        metadata::parse(&response.body, required)?;
        control.check_cancelled()?;
        let stage = Stage::new(root.try_clone()?)?;
        let staging = stage.dir()?;
        let mut file = staging.open_with(
            "selection.json",
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No),
        )?;
        file.write_all(&response.body)?;
        file.sync_all()?;
        drop(file);
        root.rename(format!("{}/selection.json", stage.name), root, &name)?;
        response.body
    };
    metadata::parse(&bytes, required)
}
fn cached_selection(root: &Dir, required: u32, name: &str) -> Result<Option<Package>, Error> {
    let cached = match root.open_with(
        name,
        OpenOptions::new().read(true).follow(FollowSymlinks::No),
    ) {
        Ok(file) => {
            let mut bytes = vec![];
            file.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
            Some(bytes)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(bytes) = cached {
        if bytes.len() <= 1024 * 1024
            && let Ok(package) = metadata::parse(&bytes, required)
        {
            return Ok(Some(package));
        }
        // This is derived metadata under the per-major lock, never user data.
        root.remove_file(name)?;
    }
    Ok(None)
}
struct Stage {
    root: Dir,
    name: String,
}
impl Stage {
    fn new(root: Dir) -> Result<Self, Error> {
        let name = format!(
            ".stage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        root.create_dir(&name)?;
        Ok(Self { root, name })
    }
    fn dir(&self) -> Result<Dir, Error> {
        Ok(self.root.open_dir_nofollow(&self.name)?)
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = self.root.remove_dir_all(&self.name);
    }
}
struct ControlledRead<R> {
    input: R,
    control: OperationControl,
}
impl<R: Read> Read for ControlledRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.control.is_cancelled() {
            return Err(std::io::Error::other("JAVA_CANCELLED"));
        }
        self.input.read(buffer)
    }
}
impl<R: Seek> Seek for ControlledRead<R> {
    fn seek(&mut self, position: std::io::SeekFrom) -> std::io::Result<u64> {
        self.input.seek(position)
    }
}
fn extract(staging: &Dir, control: OperationControl) -> Result<String, Error> {
    extract_target(staging, control, super::target::Target::host()?)
}
fn extract_target(
    staging: &Dir,
    control: OperationControl,
    platform: super::target::Target,
) -> Result<String, Error> {
    control.check_cancelled()?;
    let file = staging.open_with(
        "runtime.zip",
        OpenOptions::new().read(true).follow(FollowSymlinks::No),
    )?;
    let input = ControlledRead {
        input: file,
        control: control.clone(),
    };
    let normalized = match platform.archive {
        super::target::Archive::Zip => {
            super::archive::normalize(input, staging, &control).map(|()| Vec::new())
        }
        super::target::Archive::TarGz => super::archive::normalize_tar(input, staging, &control),
    };
    control.check_cancelled()?;
    let executable_files = normalized?;
    let input = ControlledRead {
        input: staging.open_with(
            ".normalized.zip",
            OpenOptions::new().read(true).follow(FollowSymlinks::No),
        )?,
        control: control.clone(),
    };
    let result = cobble_store::extract_zip(
        input,
        staging,
        cobble_store::ArchiveLimits {
            max_archive_bytes: 512 * 1024 * 1024,
            max_entries: 20_000,
            max_entry_bytes: 512 * 1024 * 1024,
            max_total_bytes: 2 * 1024 * 1024 * 1024,
            max_ratio: 200,
        },
    );
    control.check_cancelled()?;
    result?;
    #[cfg(unix)]
    for name in &executable_files {
        use std::os::unix::fs::PermissionsExt;
        staging
            .open_with(
                name,
                OpenOptions::new().read(true).follow(FollowSymlinks::No),
            )?
            .into_std()
            .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = executable_files;
    let mut names = vec![];
    for entry in staging.entries()? {
        let entry = entry?;
        if entry.file_name() != "runtime.zip" && entry.file_name() != ".normalized.zip" {
            names.push(entry.file_name());
        }
    }
    let [name] = names.as_slice() else {
        return Err("JAVA_ARCHIVE_LAYOUT_UNSUPPORTED".into());
    };
    let top = name.to_str().ok_or("JAVA_ARCHIVE_LAYOUT_UNSUPPORTED")?;
    SafeRelativePath::parse(top)?;
    let runtime = staging.open_dir_nofollow(top)?;
    let home = if platform.home.is_empty() {
        runtime.try_clone()?
    } else {
        runtime.open_dir_nofollow(platform.home)?
    };
    if !runtime.try_exists(platform.executable)?
        || !(home.try_exists("legal")? || home.try_exists("LICENSE")?)
    {
        return Err("JAVA_ARCHIVE_LAYOUT_UNSUPPORTED".into());
    }
    Ok(top.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-java-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn control() -> OperationControl {
        OperationControl::new(Arc::new(|_| {}))
    }
    #[test]
    fn corrupt_metadata_is_discarded_and_damaged_runtime_is_not_overwritten() {
        let temp = Temp::new();
        let root = open(&temp.0).unwrap();
        root.write(".selection-17-x64.json", b"broken-derived-cache")
            .unwrap();
        assert!(
            cached_selection(&root, 17, ".selection-17-x64.json")
                .unwrap()
                .is_none()
        );
        assert!(!root.try_exists(".selection-17-x64.json").unwrap());
        let base = "temurin-17-x64-synthetic";
        root.create_dir(base).unwrap();
        root.write(format!("{base}/owned-file"), b"preserve")
            .unwrap();
        assert_eq!(publication_name(&root, base).unwrap(), format!("{base}-r1"));
        assert_eq!(
            root.read(format!("{base}/owned-file")).unwrap(),
            b"preserve"
        );
    }
    #[test]
    fn runtime_archive_hash_mismatch_cannot_enter_verified_store() {
        let temp = Temp::new();
        let store = ContentStore::open(open(&temp.0).unwrap()).unwrap();
        let bytes = b"tampered-runtime-package";
        let expectation = ArtifactExpectation {
            expected_size: Some(bytes.len() as u64),
            max_bytes: bytes.len() as u64,
            digests: ExpectedDigests {
                sha256: Some([0xab; 32]),
                ..Default::default()
            },
            basis: IntegrityBasis::TrustedMetadata,
        };
        assert!(store.ingest(bytes.as_slice(), expectation).is_err());
        assert!(store.open_verified([0xab; 32], expectation).is_err());
    }
    #[test]
    fn managed_archive_rejects_traversal_and_cancellation() {
        let temp = Temp::new();
        let root = open(&temp.0).unwrap();
        let mut writer = zip::ZipWriter::new(root.create("runtime.zip").unwrap());
        writer
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        assert!(
            extract_target(
                &root,
                control(),
                super::super::target::Target::for_platform("windows", "x86_64").unwrap()
            )
            .is_err()
        );
        assert!(!temp.0.parent().unwrap().join("escape").exists());
        let cancelled = control();
        cancelled.cancel();
        assert!(
            extract(&root, cancelled)
                .unwrap_err()
                .is::<crate::OperationCancelled>()
        );
    }
    #[test]
    fn official_timestamp_extras_are_discarded_before_strict_ingress() {
        let temp = Temp::new();
        let root = open(&temp.0).unwrap();
        let mut writer = zip::ZipWriter::new(root.create("runtime.zip").unwrap());
        let mut options = zip::write::FullFileOptions::default();
        options
            .add_extra_field(0x5455, [1u8, 0, 0, 0, 0], false)
            .unwrap();
        writer
            .start_file("runtime/bin/java.exe", options.clone())
            .unwrap();
        writer.write_all(b"synthetic-no-execute").unwrap();
        writer.start_file("runtime/legal/LICENSE", options).unwrap();
        writer.write_all(b"license retained").unwrap();
        writer.finish().unwrap();
        assert_eq!(
            extract_target(
                &root,
                control(),
                super::super::target::Target::for_platform("windows", "x86_64").unwrap()
            )
            .unwrap(),
            "runtime"
        );
        assert_eq!(
            root.read("runtime/legal/LICENSE").unwrap(),
            b"license retained"
        );
    }
    #[test]
    fn unix_runtime_targets_extract_with_home_layout_and_executable_modes() {
        for (os, arch) in [
            ("linux", "x86_64"),
            ("macos", "x86_64"),
            ("macos", "aarch64"),
        ] {
            let target = super::super::target::Target::for_platform(os, arch).unwrap();
            let temp = Temp::new();
            let root = open(&temp.0).unwrap();
            let gzip = flate2::write::GzEncoder::new(
                root.create("runtime.zip").unwrap(),
                flate2::Compression::default(),
            );
            let mut archive = tar::Builder::new(gzip);
            for (name, bytes, mode) in [
                (
                    format!("runtime/{}", target.executable),
                    b"fixture-java".as_slice(),
                    0o755,
                ),
                (
                    format!("runtime/{}/legal/LICENSE", target.home).replace("//", "/"),
                    b"license".as_slice(),
                    0o644,
                ),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(mode);
                header.set_cksum();
                archive.append_data(&mut header, name, bytes).unwrap();
            }
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o777);
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("LICENSE").unwrap();
            header.set_cksum();
            let alias = format!("runtime/{}/legal/COPY", target.home).replace("//", "/");
            archive
                .append_data(&mut header, &alias, std::io::empty())
                .unwrap();
            archive.into_inner().unwrap().finish().unwrap();
            assert_eq!(extract_target(&root, control(), target).unwrap(), "runtime");
            assert_eq!(
                root.read(format!("runtime/{}", target.executable)).unwrap(),
                b"fixture-java"
            );
            assert_eq!(root.read(&alias).unwrap(), b"license");
            assert!(
                !std::fs::symlink_metadata(temp.0.join(&alias))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(temp.0.join("runtime").join(target.executable))
                        .unwrap()
                        .permissions()
                        .mode()
                        & 0o777,
                    0o755
                );
            }
            assert!(target.matches_arch(if arch == "aarch64" { "arm64" } else { "amd64" }));
            assert!(!target.matches_arch(if arch == "aarch64" { "amd64" } else { "arm64" }));
        }
    }
    #[tokio::test]
    #[ignore = "requires an independently downloaded official runtime and metadata in COBBLE_JAVA_ARCHIVE_PROBE"]
    async fn official_local_archive_extract_and_probe() {
        let source = std::path::PathBuf::from(
            std::env::var_os("COBBLE_JAVA_ARCHIVE_PROBE").expect("probe directory"),
        );
        let metadata_bytes = std::fs::read(source.join("linux-metadata.json")).unwrap();
        let package = metadata::parse(&metadata_bytes, 21).unwrap();
        let temp = Temp::new();
        let root = open(&temp.0).unwrap();
        let store_path = temp.0.join("cas");
        std::fs::create_dir(&store_path).unwrap();
        let store = ContentStore::open(open(&store_path).unwrap()).unwrap();
        let expectation = ArtifactExpectation {
            expected_size: Some(package.size),
            max_bytes: package.size,
            digests: ExpectedDigests {
                sha256: Some(package.sha256),
                ..Default::default()
            },
            basis: IntegrityBasis::TrustedMetadata,
        };
        let artifact = store
            .ingest(
                std::fs::File::open(source.join("runtime.tar.gz")).unwrap(),
                expectation,
            )
            .unwrap();
        store
            .copy_to(
                &artifact.artifact,
                &root,
                &SafeRelativePath::parse("runtime.zip").unwrap(),
            )
            .unwrap();
        // CAS is outside extraction staging to retain the single-root layout invariant.
        drop(store);
        std::fs::remove_dir_all(store_path).unwrap();
        let platform = super::super::target::Target::host().unwrap();
        let top = extract(&root, control()).unwrap();
        let runtime = cobble_minecraft::probe_java(&temp.0.join(&top).join(platform.executable))
            .await
            .unwrap();
        assert_eq!(runtime.major, 21);
        assert!(platform.matches_arch(&runtime.arch));
        println!(
            "official archive verified and probed: major={} arch={} sha256={:02x?}",
            runtime.major, runtime.arch, package.sha256
        );
    }
    #[test]
    fn tar_links_cannot_escape_staging() {
        for kind in [tar::EntryType::Symlink, tar::EntryType::Link] {
            let temp = Temp::new();
            let root = open(&temp.0).unwrap();
            let gzip = flate2::write::GzEncoder::new(
                root.create("runtime.zip").unwrap(),
                flate2::Compression::default(),
            );
            let mut archive = tar::Builder::new(gzip);
            let mut header = tar::Header::new_gnu();
            header.set_size(0);
            header.set_mode(0o777);
            header.set_entry_type(kind);
            header.set_link_name("../../escape").unwrap();
            header.set_cksum();
            archive
                .append_data(&mut header, "runtime/link", std::io::empty())
                .unwrap();
            archive.into_inner().unwrap().finish().unwrap();
            let target = super::super::target::Target::for_platform("linux", "x86_64").unwrap();
            assert!(
                extract_target(&root, control(), target)
                    .unwrap_err()
                    .to_string()
                    .contains("JAVA_ARCHIVE_")
            );
            assert!(!root.try_exists("runtime/link").unwrap());
        }
    }
    #[tokio::test]
    async fn precancelled_managed_install_creates_nothing() {
        let temp = Temp::new();
        let target = temp.0.join("absent");
        let cancelled = control();
        cancelled.cancel();
        assert!(
            install_at(17, &target, &temp.0.join("cache"), cancelled)
                .await
                .unwrap_err()
                .is::<crate::OperationCancelled>()
        );
        assert!(!target.exists());
    }
    #[tokio::test]
    async fn waiting_for_shared_runtime_lock_is_cancellable() {
        let temp = Temp::new();
        let root = open(&temp.0).unwrap();
        let lock = root
            .open_with(
                &format!(
                    ".install-17-{}.lock",
                    super::super::target::Target::host().unwrap().key()
                ),
                OpenOptions::new().read(true).write(true).create_new(true),
            )
            .unwrap()
            .into_std();
        lock.lock().unwrap();
        let cancelled = control();
        let trigger = cancelled.clone();
        let cache = temp.0.join("cache");
        let (result, _) = tokio::join!(install_at(17, &temp.0, &cache, cancelled), async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            trigger.cancel();
        });
        assert!(result.unwrap_err().is::<crate::OperationCancelled>());
        assert!(!temp.0.join("cache").exists());
    }
    #[tokio::test]
    #[ignore = "downloads official Java 17 into an isolated owned temporary store"]
    async fn official_java17_acquisition_probe_and_cached_reuse() {
        let probe_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/managed-java-probes/java17");
        std::fs::create_dir_all(&probe_root).unwrap();
        let target = probe_root.join("runtimes");
        let cache = probe_root.join("cache");
        let first = install_at(17, &target, &cache, control()).await.unwrap();
        assert_eq!(first.major, 17);
        let marker = first
            .executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(".cobble-runtime.json");
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(marker).unwrap()).unwrap();
        println!(
            "official managed runtime: major={} arch={} release={} package={} size={} hash={}",
            first.major,
            first.arch,
            value["package"]["release"],
            value["package"]["url"],
            value["package"]["size"],
            value["package"]["sha256"]
        );
        let second = install_at(17, &target, &cache, control()).await.unwrap();
        assert_eq!(first.executable, second.executable);
        let version = tokio::process::Command::new(&first.executable)
            .arg("-version")
            .output()
            .await
            .unwrap();
        assert!(version.status.success());
        println!("{}", String::from_utf8_lossy(&version.stderr));
    }
}

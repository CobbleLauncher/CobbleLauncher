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
    std::fs::create_dir_all(path)?;
    let root = open(path)?;
    let lock = root
        .open_with(
            format!(".install-{required}-x64.lock"),
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
        if name
            .to_str()
            .is_some_and(|name| name.starts_with(&format!("temurin-{required}-x64-")))
            && root.open_dir_nofollow(&name).is_ok()
            && let Ok(runtime) =
                cobble_minecraft::probe_java(&path.join(&name).join("bin/java.exe")).await
            && runtime.major == required
            && matches!(runtime.arch.as_str(), "amd64" | "x86_64" | "x64")
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
                    "temurin-ga-zip-v1",
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
    let executable = path.join(&stage.name).join(&top).join("bin/java.exe");
    let runtime = cobble_minecraft::probe_java(&executable).await?;
    if runtime.major != required || !matches!(runtime.arch.as_str(), "amd64" | "x86_64" | "x64") {
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
    let base = format!("temurin-{required}-x64-{}", &suffix[..16]);
    // Never replace files which may still belong to a running JVM. A damaged
    // managed publication gets a separate verified replacement, bounded here.
    let target = publication_name(&stage.root, &base)?;
    stage
        .root
        .rename(format!("{}/{top}", stage.name), &stage.root, &target)?;
    phase(&control, "java-verify", 1, 1);
    Ok(JavaRuntime {
        executable: std::fs::canonicalize(path.join(target).join("bin/java.exe"))?,
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
    let name = format!(".selection-{required}-x64.json");
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
    control.check_cancelled()?;
    let file = staging.open_with(
        "runtime.zip",
        OpenOptions::new().read(true).follow(FollowSymlinks::No),
    )?;
    let input = ControlledRead {
        input: file,
        control: control.clone(),
    };
    let normalized = super::archive::normalize(input, staging, &control);
    control.check_cancelled()?;
    normalized?;
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
    if !runtime.try_exists("bin/java.exe")?
        || !(runtime.try_exists("legal")? || runtime.try_exists("LICENSE")?)
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
        assert!(extract(&root, control()).is_err());
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
        assert_eq!(extract(&root, control()).unwrap(), "runtime");
        assert_eq!(
            root.read("runtime/legal/LICENSE").unwrap(),
            b"license retained"
        );
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
                ".install-17-x64.lock",
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

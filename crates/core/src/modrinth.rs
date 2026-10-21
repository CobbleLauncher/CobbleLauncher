// SPDX-License-Identifier: MIT
//! Install an already resolved official Modrinth plan using the shared CAS.
//! The caller owns the instance write lease and must refuse running instances.
//! This additive slice preserves files on failure; retries finish the pinned plan.
use crate::InstallProgress;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{ModrinthPlan, ResolvedMod};
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, DownloadAuthority, DownloadManager, DownloadRequest,
    TransferCancellation,
};
use cobble_store::{
    ArtifactExpectation, ContentStore, DownloadStaging, ExpectedDigests, IntegrityBasis,
    SafeRelativePath, VerifiedArtifact,
};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Write},
    path::Path,
    sync::Arc,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
pub(super) const MARKER: &str = ".cobble-modrinth.json";
const MAX_FILE: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ModrinthInstallOutcome {
    pub installed: usize,
    /// Existing matching files (including intentionally disabled files).
    pub reused: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(super) struct ManagedMod {
    pub version_id: String,
    pub filename: String,
    pub sha1: String,
}
pub(super) type Provenance = BTreeMap<String, ManagedMod>;

pub(super) struct ModDownloads {
    pub content: ContentStore,
    index: Dir,
    downloader: DownloadManager,
}

impl ModDownloads {
    pub fn open() -> Result<Self, Error> {
        let cache = dirs::cache_dir()
            .ok_or("could not locate user cache")?
            .join("CobbleLauncher");
        for name in ["downloads", "content", "sha1"] {
            std::fs::create_dir_all(cache.join(name))?;
        }
        let open =
            |name: &str| Dir::open_ambient_dir(cache.join(name), cap_std::ambient_authority());
        Ok(Self {
            content: ContentStore::open(open("content")?)?,
            index: open("sha1")?,
            downloader: DownloadManager::new(ArtifactDownloader::new(
                Broker::new(BrokerConfig {
                    total_timeout: std::time::Duration::from_secs(300),
                    ..BrokerConfig::default()
                })?,
                DownloadStaging::open(open("downloads")?)?,
                ContentStore::open(open("content")?)?,
            )),
        })
    }

    pub async fn fetch(&self, selected: &ResolvedMod) -> Result<VerifiedArtifact, Error> {
        let expected = expectation(selected)?;
        let artifact = match crate::cache::lookup(&self.content, &self.index, expected)? {
            Some(artifact) => artifact,
            None => {
                let request = DownloadRequest::new(
                    &selected.file.url,
                    expected,
                    DownloadAuthority::same_origin(
                        "modrinth",
                        "mod-install",
                        "official-cdn-v1",
                        None,
                    )?,
                    None,
                )?;
                self.downloader
                    .download(request, TransferCancellation::default())
                    .await?
                    .artifact
                    .clone()
            }
        };
        let _ = crate::cache::record(&self.index, &artifact);
        Ok(artifact)
    }
}

/// Downloads the complete plan before publishing any JAR. No game is launched.
/// Partial publication is recorded before copies and can be retried; this is
/// not a multi-file atomic transaction or an update implementation.
pub async fn install_modrinth_plan(
    plan: &ModrinthPlan,
    directory: &Path,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<ModrinthInstallOutcome, Error> {
    validate_plan(plan)?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let mut provenance = read_provenance(&root)?;
    let mods = match root.open_dir_nofollow("mods") {
        Ok(mods) => Some(mods),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(mods) = &mods {
        preflight(plan, mods, &mut provenance)?;
    }
    let downloads = ModDownloads::open()?;
    let mut artifacts: Vec<Option<VerifiedArtifact>> = Vec::new();
    let mut reused = 0;
    let total = plan.mods.len();
    on_progress(progress("mod-download", 0, total, reused));
    for (i, selected) in plan.mods.iter().enumerate() {
        let existing = mods
            .as_ref()
            .map(|mods| existing_matches(mods, selected))
            .transpose()?
            .unwrap_or(false);
        if existing {
            reused += 1;
            artifacts.push(None);
        } else {
            let artifact = downloads.fetch(selected).await?;
            artifacts.push(Some(artifact));
        }
        on_progress(progress("mod-download", i + 1, total, reused));
    }
    root.create_dir_all("mods")?;
    let mods = root.open_dir_nofollow("mods")?;
    // Recheck after downloads, before recording or publishing any change.
    preflight(plan, &mods, &mut provenance)?;
    for selected in &plan.mods {
        provenance.insert(
            selected.project_id.clone(),
            ManagedMod {
                version_id: selected.version_id.clone(),
                filename: selected.file.filename.clone(),
                sha1: selected.file.sha1.clone(),
            },
        );
    }
    write_provenance(&root, &provenance)?;
    let mut installed = 0;
    let mut existing = 0;
    on_progress(progress("mod-install", 0, total, existing));
    for (i, (selected, artifact)) in plan.mods.iter().zip(artifacts).enumerate() {
        if existing_matches(&mods, selected)? {
            existing += 1;
        } else {
            let artifact =
                artifact.ok_or("an existing mod disappeared during installation; retry")?;
            downloads.content.copy_to(
                &artifact,
                &mods,
                &SafeRelativePath::parse(&selected.file.filename)?,
            )?;
            installed += 1;
        }
        on_progress(progress("mod-install", i + 1, total, existing));
    }
    on_progress(progress("complete", total, total, existing));
    Ok(ModrinthInstallOutcome {
        installed,
        reused: existing,
    })
}

fn progress(phase: &str, completed: usize, total: usize, reused: usize) -> InstallProgress {
    InstallProgress {
        phase: phase.into(),
        completed,
        total,
        reused,
    }
}

pub(super) fn validate_plan(plan: &ModrinthPlan) -> Result<(), Error> {
    if plan.mods.is_empty() || plan.mods.len() > 256 {
        return Err("invalid Modrinth plan length".into());
    }
    let mut names = BTreeSet::new();
    let mut projects = BTreeSet::new();
    for selected in &plan.mods {
        let name = &selected.file.filename;
        let path = SafeRelativePath::parse(name)?;
        if path.as_str().contains('/')
            || name.starts_with('.')
            || !name.to_ascii_lowercase().ends_with(".jar")
            || !names.insert(name.to_ascii_lowercase())
            || !projects.insert(&selected.project_id)
        {
            return Err("duplicate or unsafe Modrinth filename/project".into());
        }
        if !selected
            .file
            .url
            .starts_with("https://cdn.modrinth.com/data/")
        {
            return Err("unsupported Modrinth artifact origin".into());
        }
        expectation(selected)?;
    }
    Ok(())
}

fn digest<const N: usize>(value: &str) -> Result<[u8; N], Error> {
    if value.len() != N * 2 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid Modrinth digest".into());
    }
    let mut result = [0; N];
    for (i, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)?;
    }
    Ok(result)
}

fn expectation(selected: &ResolvedMod) -> Result<ArtifactExpectation, Error> {
    if selected.file.size == 0 || selected.file.size > MAX_FILE {
        return Err("Modrinth file size exceeds supported limit".into());
    }
    Ok(ArtifactExpectation {
        expected_size: Some(selected.file.size),
        max_bytes: selected.file.size,
        basis: IntegrityBasis::TrustedMetadata,
        digests: ExpectedDigests {
            sha1: Some(digest(&selected.file.sha1)?),
            sha512: selected.file.sha512.as_deref().map(digest).transpose()?,
            ..ExpectedDigests::default()
        },
    })
}

fn preflight(plan: &ModrinthPlan, mods: &Dir, provenance: &mut Provenance) -> Result<(), Error> {
    // Deleted local files release their managed identity; disabled files retain it.
    let mut keep = Provenance::new();
    for (project, entry) in provenance.iter() {
        let name = SafeRelativePath::parse(&entry.filename)?;
        if name.as_str().contains('/') {
            return Err("invalid managed mod basename".into());
        }
        if mods.symlink_metadata(&entry.filename).is_ok()
            || mods
                .symlink_metadata(format!("{}.disabled", entry.filename))
                .is_ok()
        {
            keep.insert(project.clone(), entry.clone());
        }
    }
    *provenance = keep;
    let mut disk_names = BTreeMap::new();
    for entry in mods.entries()? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        disk_names.insert(name.to_ascii_lowercase(), name);
    }
    for selected in &plan.mods {
        if let Some(old) = provenance.get(&selected.project_id)
            && (old.version_id != selected.version_id
                || old.filename != selected.file.filename
                || !old.sha1.eq_ignore_ascii_case(&selected.file.sha1))
        {
            return Err(format!(
                "{} already has another managed version; remove it manually before installing",
                selected.name
            )
            .into());
        }
        for name in [
            &selected.file.filename,
            &format!("{}.disabled", selected.file.filename),
        ] {
            if disk_names
                .get(&name.to_ascii_lowercase())
                .is_some_and(|disk| disk != name)
            {
                return Err(format!("case-insensitive mod filename collision: {name}").into());
            }
        }
        existing_matches(mods, selected)?;
    }
    Ok(())
}

fn existing_matches(mods: &Dir, selected: &ResolvedMod) -> Result<bool, Error> {
    let mut found = false;
    for name in [
        selected.file.filename.clone(),
        format!("{}.disabled", selected.file.filename),
    ] {
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let mut file = match mods.open_with(&name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() != selected.file.size {
            return Err(format!("existing mod differs: {name}; remove it manually first").into());
        }
        let mut sha1 = Sha1::new();
        let mut sha512 = Sha512::new();
        let mut buffer = [0; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            sha1.update(&buffer[..count]);
            sha512.update(&buffer[..count]);
        }
        if sha1.finalize().as_slice() != digest::<20>(&selected.file.sha1)? {
            return Err(
                format!("existing mod hash differs: {name}; remove it manually first").into(),
            );
        }
        if let Some(expected) = &selected.file.sha512
            && sha512.finalize().as_slice() != digest::<64>(expected)?
        {
            return Err(format!("existing mod SHA-512 differs: {name}").into());
        }
        if found {
            return Err(
                "both enabled and disabled copies exist; resolve duplicate manually".into(),
            );
        }
        found = true;
    }
    Ok(found)
}

pub(super) fn read_provenance(root: &Dir) -> Result<Provenance, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    match root.open_with(MARKER, &options) {
        Ok(file) => Ok(serde_json::from_reader(file.take(1024 * 1024))?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Provenance::new()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn write_provenance(root: &Dir, provenance: &Provenance) -> Result<(), Error> {
    let temporary = ".cobble-modrinth.json.tmp";
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(temporary, &options)?;
    serde_json::to_writer_pretty(&mut file, provenance)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(temporary, root, MARKER)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_content::ModrinthFile;
    use std::{path::PathBuf, sync::Mutex};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cobble-modrinth-{stamp}-{}",
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("mods")).unwrap();
            Self(path)
        }
        fn mods(&self) -> Dir {
            Dir::open_ambient_dir(self.0.join("mods"), cap_std::ambient_authority()).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn plan() -> ModrinthPlan {
        ModrinthPlan {
            project_id: "synthetic".into(),
            version_id: "one".into(),
            mods: vec![ResolvedMod {
                project_id: "synthetic".into(),
                version_id: "one".into(),
                name: "Synthetic".into(),
                version_number: "1".into(),
                required_by: None,
                file: ModrinthFile {
                    url: "https://cdn.modrinth.com/data/synthetic/versions/one/sample.jar".into(),
                    filename: "sample.jar".into(),
                    size: 7,
                    sha1: format!("{:x}", Sha1::digest(b"fixture")),
                    sha512: Some(format!("{:x}", Sha512::digest(b"fixture"))),
                },
            }],
        }
    }
    #[test]
    fn exact_local_import_and_disabled_file_are_reused() {
        let fixture = Fixture::new();
        let plan = plan();
        std::fs::write(fixture.0.join("mods/sample.jar"), b"fixture").unwrap();
        assert!(existing_matches(&fixture.mods(), &plan.mods[0]).unwrap());
        std::fs::rename(
            fixture.0.join("mods/sample.jar"),
            fixture.0.join("mods/sample.jar.disabled"),
        )
        .unwrap();
        assert!(existing_matches(&fixture.mods(), &plan.mods[0]).unwrap());
    }
    #[test]
    fn collision_and_case_collision_refuse_whole_plan() {
        let fixture = Fixture::new();
        let plan = plan();
        std::fs::write(fixture.0.join("mods/sample.jar"), b"changed").unwrap();
        assert!(preflight(&plan, &fixture.mods(), &mut Provenance::new()).is_err());
        std::fs::remove_file(fixture.0.join("mods/sample.jar")).unwrap();
        std::fs::write(fixture.0.join("mods/SAMPLE.jar"), b"fixture").unwrap();
        assert!(preflight(&plan, &fixture.mods(), &mut Provenance::new()).is_err());
    }
    #[test]
    fn stronger_digest_is_checked_for_local_import() {
        let fixture = Fixture::new();
        let mut plan = plan();
        std::fs::write(fixture.0.join("mods/sample.jar"), b"fixture").unwrap();
        plan.mods[0].file.sha512 = Some("0".repeat(128));
        assert!(existing_matches(&fixture.mods(), &plan.mods[0]).is_err());
    }
    #[test]
    fn managed_version_blocks_until_manual_removal_including_disabled() {
        let fixture = Fixture::new();
        let plan = plan();
        let mut provenance = Provenance::from([(
            "synthetic".into(),
            ManagedMod {
                version_id: "old".into(),
                filename: "old.jar".into(),
                sha1: "0".repeat(40),
            },
        )]);
        std::fs::write(fixture.0.join("mods/old.jar.disabled"), b"old").unwrap();
        assert!(preflight(&plan, &fixture.mods(), &mut provenance).is_err());
        std::fs::remove_file(fixture.0.join("mods/old.jar.disabled")).unwrap();
        preflight(&plan, &fixture.mods(), &mut provenance).unwrap();
        assert!(provenance.is_empty());
    }
    #[test]
    fn offline_retry_preserves_disabled_mod_and_reports_completion() {
        let fixture = Fixture::new();
        let plan = plan();
        std::fs::write(fixture.0.join("mods/sample.jar.disabled"), b"fixture").unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let outcome = runtime
            .block_on(install_modrinth_plan(
                &plan,
                &fixture.0,
                Arc::new(move |event| captured.lock().unwrap().push(event)),
            ))
            .unwrap();
        assert_eq!(outcome.installed, 0);
        assert_eq!(outcome.reused, 1);
        assert!(!fixture.0.join("mods/sample.jar").exists());
        assert_eq!(
            std::fs::read(fixture.0.join("mods/sample.jar.disabled")).unwrap(),
            b"fixture"
        );
        let events = events.lock().unwrap();
        let complete = events.last().unwrap();
        assert_eq!(
            (
                &*complete.phase,
                complete.completed,
                complete.total,
                complete.reused
            ),
            ("complete", 1, 1, 1)
        );
        let root = Dir::open_ambient_dir(&fixture.0, cap_std::ambient_authority()).unwrap();
        assert_eq!(
            read_provenance(&root).unwrap()["synthetic"].version_id,
            "one"
        );
    }
}

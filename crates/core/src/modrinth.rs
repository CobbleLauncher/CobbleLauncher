// SPDX-License-Identifier: MIT
//! Install an already resolved official Modrinth plan using the shared CAS.
//! The caller owns the instance write lease and must refuse running instances.
//! This additive slice preserves files on failure; retries finish the pinned plan.
use crate::InstallProgress;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{ModrinthFile, ModrinthPlan, ResolvedMod};
use cobble_net::{
    ArtifactDownloader, Broker, BrokerConfig, CredentialId, CredentialPartition, CredentialScope,
    DownloadAuthority, DownloadManager, DownloadRequest, ProviderCredentials,
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
const INSTALL_PENDING: &str = ".cobble-mod-install.json";
const MAX_FILE: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModProvider {
    #[default]
    #[serde(rename = "modrinth")]
    Modrinth,
    #[serde(rename = "curseforge")]
    CurseForge,
}
impl ModProvider {
    fn marker(self) -> &'static str {
        match self {
            Self::Modrinth => MARKER,
            Self::CurseForge => ".cobble-curseforge.json",
        }
    }
    fn pending(self) -> &'static str {
        match self {
            Self::Modrinth => INSTALL_PENDING,
            Self::CurseForge => ".cobble-curseforge-install.json",
        }
    }
}

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
    provider: ModProvider,
    credential: Option<CredentialId>,
}

impl ModDownloads {
    pub(super) fn concurrency(&self) -> usize {
        self.downloader.concurrency()
    }
    pub fn open() -> Result<Self, Error> {
        Self::open_for(ModProvider::Modrinth, None)
    }
    pub(super) fn open_for(provider: ModProvider, api_key: Option<String>) -> Result<Self, Error> {
        let cache = dirs::cache_dir()
            .ok_or("could not locate user cache")?
            .join("CobbleLauncher");
        Self::open_at(&cache, provider, api_key)
    }
    pub(super) fn open_at(
        cache: &Path,
        provider: ModProvider,
        api_key: Option<String>,
    ) -> Result<Self, Error> {
        for name in ["downloads", "content", "sha1"] {
            std::fs::create_dir_all(cache.join(name))?;
        }
        let open =
            |name: &str| Dir::open_ambient_dir(cache.join(name), cap_std::ambient_authority());
        let broker = Broker::new(BrokerConfig {
            total_timeout: std::time::Duration::from_secs(300),
            ..BrokerConfig::default()
        })?;
        let credential = if provider == ModProvider::CurseForge {
            Some(broker.register_download_credentials(
                CredentialScope::new("https://edge.forgecdn.net", "/files/")?,
                ProviderCredentials {
                    bearer: None,
                    api_key: Some(api_key.ok_or("CurseForge credential unavailable")?),
                    cookie: None,
                },
                CredentialPartition::new("curseforge-launcher-download-v1")?,
            )?)
        } else {
            None
        };
        Ok(Self {
            content: ContentStore::open(open("content")?)?,
            index: open("sha1")?,
            downloader: DownloadManager::new(ArtifactDownloader::new(
                broker,
                DownloadStaging::open(open("downloads")?)?,
                ContentStore::open(open("content")?)?,
            )),
            provider,
            credential,
        })
    }

    pub async fn fetch_file_controlled(
        &self,
        file: &ModrinthFile,
        control: &crate::OperationControl,
    ) -> Result<VerifiedArtifact, Error> {
        control.check_cancelled()?;
        if self.provider == ModProvider::CurseForge
            && !file.url.is_empty()
            && !cobble_content::curseforge::official_file_url(&file.url)
        {
            return Err("unsupported CurseForge artifact origin".into());
        }
        let expected = file_expectation(file)?;
        let artifact = match crate::cache::lookup(&self.content, &self.index, expected)? {
            Some(artifact) => artifact,
            None => {
                if file.url.is_empty() {
                    return Err("请选择并验证作者要求手动下载的文件".into());
                }
                let credential = if self.provider == ModProvider::CurseForge
                    && file.url.starts_with("https://edge.forgecdn.net/files/")
                {
                    self.credential.clone()
                } else {
                    None
                };
                let request = DownloadRequest::new(
                    &file.url,
                    expected,
                    DownloadAuthority::same_origin(
                        if self.provider == ModProvider::CurseForge {
                            "curseforge"
                        } else {
                            "modrinth"
                        },
                        "mod-install",
                        "official-cdn-v1",
                        credential
                            .as_ref()
                            .map(|_| CredentialPartition::new("curseforge-launcher-download-v1"))
                            .transpose()?,
                    )?,
                    credential,
                )?;
                let request = if self.provider == ModProvider::CurseForge {
                    request.with_curseforge_redirects()?
                } else {
                    request
                };
                let result = self
                    .downloader
                    .download(request, control.cancellation())
                    .await;
                self.downloader.wait_idle().await;
                control.check_cancelled()?;
                result?.artifact.clone()
            }
        };
        let _ = crate::cache::record(&self.index, &artifact);
        Ok(artifact)
    }

    pub(super) fn contains(&self, file: &ModrinthFile) -> Result<bool, Error> {
        self.contains_expectation(file_expectation(file)?)
    }

    pub(super) fn contains_expectation(
        &self,
        expected: ArtifactExpectation,
    ) -> Result<bool, Error> {
        Ok(crate::cache::lookup(&self.content, &self.index, expected)?.is_some())
    }
    pub(super) fn ingest_manual(
        &self,
        file: &ModrinthFile,
        input: std::fs::File,
    ) -> Result<(), Error> {
        let artifact = self
            .content
            .ingest(input, file_expectation(file)?)?
            .artifact;
        crate::cache::record(&self.index, &artifact)?;
        Ok(())
    }
}

/// Explicit task retry only: recognize an already published exact artifact.
pub(super) fn matches_downloaded_file(path: &Path, selected: &ModrinthFile) -> Result<bool, Error> {
    file_expectation(selected)?;
    let root = Dir::open_ambient_dir(
        path.parent().ok_or("Artifact has no parent")?,
        cap_std::ambient_authority(),
    )?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let mut input = match root.open_with(
        path.file_name().ok_or("Artifact has no filename")?,
        &options,
    ) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !input.metadata()?.is_file() || input.metadata()?.len() != selected.size {
        return Ok(false);
    }
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut buffer = [0; 65536];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        sha1.update(&buffer[..count]);
        sha512.update(&buffer[..count]);
    }
    Ok(
        format!("{:x}", sha1.finalize()).eq_ignore_ascii_case(&selected.sha1)
            && selected.sha512.as_ref().is_none_or(|expected| {
                format!("{:x}", sha512.finalize()).eq_ignore_ascii_case(expected)
            }),
    )
}

/// Downloads the complete plan before publishing any JAR. No game is launched.
/// Partial publication is recorded before copies and can be retried; this is
/// not a multi-file atomic transaction or an update implementation.
pub async fn install_modrinth_plan(
    plan: &ModrinthPlan,
    directory: &Path,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<ModrinthInstallOutcome, Error> {
    install_modrinth_plan_controlled(plan, directory, crate::OperationControl::new(on_progress))
        .await
}

pub async fn install_modrinth_plan_controlled(
    plan: &ModrinthPlan,
    directory: &Path,
    control: crate::OperationControl,
) -> Result<ModrinthInstallOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    validate_plan(plan)?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    check_pending_plan(&root, plan)?;
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
        control.check_cancelled()?;
        let existing = mods
            .as_ref()
            .map(|mods| existing_matches(mods, selected))
            .transpose()?
            .unwrap_or(false);
        if existing {
            reused += 1;
            artifacts.push(None);
        } else {
            let artifact = downloads
                .fetch_file_controlled(&selected.file, &control)
                .await?;
            artifacts.push(Some(artifact));
        }
        on_progress(progress("mod-download", i + 1, total, reused));
    }
    control.check_cancelled()?;
    let plan = plan.clone();
    tokio::task::spawn_blocking(move || {
        commit_mod_plan(&root, &plan, &control, |index, selected, mods| {
            let artifact = artifacts[index]
                .as_ref()
                .ok_or("an existing mod disappeared during installation; retry")?;
            downloads.content.copy_to(
                artifact,
                mods,
                &SafeRelativePath::parse(&selected.file.filename)?,
            )?;
            Ok(())
        })
    })
    .await?
}

fn plan_identity(plan: &ModrinthPlan) -> Provenance {
    plan.mods
        .iter()
        .map(|selected| {
            (
                selected.project_id.clone(),
                ManagedMod {
                    version_id: selected.version_id.clone(),
                    filename: selected.file.filename.clone(),
                    sha1: selected.file.sha1.clone(),
                },
            )
        })
        .collect()
}

fn check_pending_plan(root: &Dir, plan: &ModrinthPlan) -> Result<bool, Error> {
    check_provider_pending_plan(root, plan, ModProvider::Modrinth)
}
pub(super) fn check_provider_pending_plan(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
) -> Result<bool, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let input = match root.open_with(provider.pending(), &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let pending: Provenance = serde_json::from_reader(input.take(4 * 1024 * 1024))?;
    if pending != plan_identity(plan) {
        return Err("另一个 Mod 安装尚未完成，请先重试原安装任务".into());
    }
    Ok(true)
}

fn record_provider_pending_plan(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
) -> Result<(), Error> {
    if check_provider_pending_plan(root, plan, provider)? {
        return Ok(());
    }
    let temporary = format!("{}.tmp", provider.pending());
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(&temporary, &options)?;
    serde_json::to_writer(&mut file, &plan_identity(plan))?;
    file.sync_all()?;
    drop(file);
    root.rename(&temporary, root, provider.pending())?;
    Ok(())
}

pub(crate) fn ensure_mod_install_complete(directory: &Path) -> Result<(), Error> {
    for provider in [ModProvider::Modrinth, ModProvider::CurseForge] {
        match std::fs::symlink_metadata(directory.join(provider.pending())) {
            Ok(_) => return Err("Mod 安装尚未完成，请先重试原安装任务".into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn commit_mod_plan(
    root: &Dir,
    plan: &ModrinthPlan,
    control: &crate::OperationControl,
    publish: impl FnMut(usize, &ResolvedMod, &Dir) -> Result<(), Error>,
) -> Result<ModrinthInstallOutcome, Error> {
    commit_provider_mod_plan(root, plan, ModProvider::Modrinth, control, publish)
}
pub(super) fn commit_provider_mod_plan(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
    control: &crate::OperationControl,
    mut publish: impl FnMut(usize, &ResolvedMod, &Dir) -> Result<(), Error>,
) -> Result<ModrinthInstallOutcome, Error> {
    check_provider_ownership(root, plan, provider)?;
    let on_progress = control.progress();
    let total = plan.mods.len();
    on_progress(progress("mod-install", 0, total, 0));
    control.check_cancelled()?;
    check_provider_pending_plan(root, plan, provider)?;
    root.create_dir_all("mods")?;
    let mods = root.open_dir_nofollow("mods")?;
    let mut provenance = read_provider_provenance(root, provider)?;
    // Recheck after downloads, before recording or publishing any change.
    preflight(plan, &mods, &mut provenance)?;
    control.check_cancelled()?;
    let history_id = crate::modupdates::history::begin_install(root, plan, provider)?;
    // From this point publication completes or leaves a fixed retry marker.
    // Cancellation during commit is observed by the owner after successful work,
    // not by abandoning a root whose required dependencies are still missing.
    record_provider_pending_plan(root, plan, provider)?;
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
    write_provider_provenance(root, &provenance, provider)?;
    let mut installed = 0;
    let mut existing = 0;
    for (i, selected) in plan.mods.iter().enumerate() {
        if existing_matches(&mods, selected)? {
            existing += 1;
        } else {
            publish(i, selected, &mods)?;
            installed += 1;
        }
        on_progress(progress("mod-install", i + 1, total, existing));
    }
    crate::modupdates::history::finish_install(root, &history_id)?;
    root.remove_file(provider.pending())?;
    on_progress(progress("complete", total, total, existing));
    Ok(ModrinthInstallOutcome {
        installed,
        reused: existing,
    })
}

pub(super) fn check_provider_ownership(
    root: &Dir,
    plan: &ModrinthPlan,
    provider: ModProvider,
) -> Result<(), Error> {
    let other = match provider {
        ModProvider::Modrinth => ModProvider::CurseForge,
        ModProvider::CurseForge => ModProvider::Modrinth,
    };
    let provenance = read_provider_provenance(root, other)?;
    if plan.mods.iter().any(|selected| {
        provenance.values().any(|managed| {
            managed
                .filename
                .eq_ignore_ascii_case(&selected.file.filename)
        })
    }) {
        return Err("所选文件已由另一个内容来源管理，请先移除对应 Mod".into());
    }
    Ok(())
}

fn progress(phase: &str, completed: usize, total: usize, reused: usize) -> InstallProgress {
    InstallProgress {
        detail: None,
        phase: phase.into(),
        completed,
        total,
        reused,
    }
}

pub(super) fn validate_plan(plan: &ModrinthPlan) -> Result<(), Error> {
    validate_provider_plan(plan, ModProvider::Modrinth)
}
pub(super) fn validate_provider_plan(
    plan: &ModrinthPlan,
    provider: ModProvider,
) -> Result<(), Error> {
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
        let authorized = match provider {
            ModProvider::Modrinth => selected
                .file
                .url
                .starts_with("https://cdn.modrinth.com/data/"),
            ModProvider::CurseForge => {
                selected.file.url.is_empty()
                    || cobble_content::curseforge::official_file_url(&selected.file.url)
            }
        };
        if !authorized {
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
    file_expectation(&selected.file)
}

pub(super) fn file_expectation(file: &ModrinthFile) -> Result<ArtifactExpectation, Error> {
    if file.size == 0 || file.size > MAX_FILE {
        return Err("Modrinth file size exceeds supported limit".into());
    }
    Ok(ArtifactExpectation {
        expected_size: Some(file.size),
        max_bytes: file.size,
        basis: IntegrityBasis::TrustedMetadata,
        digests: ExpectedDigests {
            sha1: Some(digest(&file.sha1)?),
            sha512: file.sha512.as_deref().map(digest).transpose()?,
            ..ExpectedDigests::default()
        },
    })
}

pub(super) fn preflight(
    plan: &ModrinthPlan,
    mods: &Dir,
    provenance: &mut Provenance,
) -> Result<(), Error> {
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

pub(super) fn existing_matches(mods: &Dir, selected: &ResolvedMod) -> Result<bool, Error> {
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
    read_provider_provenance(root, ModProvider::Modrinth)
}
pub(super) fn read_provider_provenance(
    root: &Dir,
    provider: ModProvider,
) -> Result<Provenance, Error> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    match root.open_with(provider.marker(), &options) {
        Ok(file) => Ok(serde_json::from_reader(file.take(1024 * 1024))?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Provenance::new()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn write_provenance(root: &Dir, provenance: &Provenance) -> Result<(), Error> {
    write_provider_provenance(root, provenance, ModProvider::Modrinth)
}
pub(super) fn write_provider_provenance(
    root: &Dir,
    provenance: &Provenance,
    provider: ModProvider,
) -> Result<(), Error> {
    let temporary = format!("{}.tmp", provider.marker());
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(&temporary, &options)?;
    serde_json::to_writer_pretty(&mut file, provenance)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(&temporary, root, provider.marker())?;
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

    fn dependency_plan() -> ModrinthPlan {
        let mut plan = plan();
        let mut dependency = plan.mods[0].clone();
        dependency.project_id = "dependency".into();
        dependency.version_id = "dependency-one".into();
        dependency.name = "Required dependency".into();
        dependency.required_by = Some("synthetic".into());
        dependency.file.filename = "dependency.jar".into();
        plan.mods.push(dependency);
        plan
    }

    #[test]
    fn install_history_undo_keeps_reused_mod_and_removes_new_dependency() {
        let fixture = Fixture::new();
        let root = Dir::open_ambient_dir(&fixture.0, cap_std::ambient_authority()).unwrap();
        fixture
            .mods()
            .write("sample.jar.disabled", b"fixture")
            .unwrap();
        let control = crate::OperationControl::new(Arc::new(|_| {}));
        let result = commit_mod_plan(&root, &dependency_plan(), &control, |_, selected, mods| {
            mods.write(&selected.file.filename, b"fixture")?;
            Ok(())
        })
        .unwrap();
        assert_eq!((result.installed, result.reused), (1, 1));
        let history = crate::list_mod_operation_history(&fixture.0).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].files.len(), 1);
        crate::undo_mod_operation(&fixture.0, &history[0].operation_id).unwrap();
        assert_eq!(
            fixture.mods().read("sample.jar.disabled").unwrap(),
            b"fixture"
        );
        assert!(!fixture.mods().exists("dependency.jar"));
        assert_eq!(read_provenance(&root).unwrap().len(), 1);
    }

    fn cancel_on_install_progress(completed: usize) -> crate::OperationControl {
        let token = Arc::new(Mutex::new(None::<cobble_net::TransferCancellation>));
        let observed = token.clone();
        let control = crate::OperationControl::new(Arc::new(move |event| {
            if event.phase == "mod-install" && event.completed == completed {
                observed.lock().unwrap().as_ref().unwrap().cancel();
            }
        }));
        *token.lock().unwrap() = Some(control.cancellation());
        control
    }

    #[test]
    fn cancel_before_additive_commit_leaves_no_marker_or_partial_provenance() {
        let fixture = Fixture::new();
        let root = Dir::open_ambient_dir(&fixture.0, cap_std::ambient_authority()).unwrap();
        let control = cancel_on_install_progress(0);
        let result = commit_mod_plan(&root, &dependency_plan(), &control, |_, _, _| {
            panic!("publication must not start")
        });
        assert!(result.unwrap_err().is::<crate::OperationCancelled>());
        assert!(root.symlink_metadata(INSTALL_PENDING).is_err());
        assert!(read_provenance(&root).unwrap().is_empty());
        assert!(fixture.mods().entries().unwrap().next().is_none());
        ensure_mod_install_complete(&fixture.0).unwrap();
    }

    #[test]
    fn cancellation_after_first_additive_jar_finishes_required_dependencies() {
        let fixture = Fixture::new();
        let root = Dir::open_ambient_dir(&fixture.0, cap_std::ambient_authority()).unwrap();
        let control = cancel_on_install_progress(1);
        let result = commit_mod_plan(&root, &dependency_plan(), &control, |_, selected, mods| {
            mods.write(&selected.file.filename, b"fixture")?;
            Ok(())
        })
        .unwrap();
        assert!(control.is_cancelled());
        assert_eq!((result.installed, result.reused), (2, 0));
        assert_eq!(fixture.mods().read("sample.jar").unwrap(), b"fixture");
        assert_eq!(fixture.mods().read("dependency.jar").unwrap(), b"fixture");
        assert_eq!(read_provenance(&root).unwrap().len(), 2);
        ensure_mod_install_complete(&fixture.0).unwrap();
    }

    #[test]
    fn interrupted_additive_plan_blocks_launch_and_only_same_plan_can_resume() {
        let fixture = Fixture::new();
        let root = Dir::open_ambient_dir(&fixture.0, cap_std::ambient_authority()).unwrap();
        let plan = dependency_plan();
        let control = crate::OperationControl::new(Arc::new(|_| {}));
        let result = commit_mod_plan(&root, &plan, &control, |index, selected, mods| {
            if index == 1 {
                return Err("synthetic interrupted disk copy".into());
            }
            mods.write(&selected.file.filename, b"fixture")?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(check_pending_plan(&root, &plan).unwrap());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let error = runtime
            .block_on(crate::prepare_launch("1.21.1", &fixture.0))
            .unwrap_err();
        assert!(error.to_string().contains("Mod 安装尚未完成"));
        let mut different = plan.clone();
        different.mods[0].version_id = "another-version".into();
        assert!(check_pending_plan(&root, &different).is_err());
        let result = commit_mod_plan(&root, &plan, &control, |_, selected, mods| {
            mods.write(&selected.file.filename, b"fixture")?;
            Ok(())
        })
        .unwrap();
        assert_eq!((result.installed, result.reused), (1, 1));
        ensure_mod_install_complete(&fixture.0).unwrap();
        assert_eq!(fixture.mods().read("dependency.jar").unwrap(), b"fixture");
    }

    #[test]
    fn task_retry_accepts_only_exact_published_artifact_bytes() {
        let fixture = Fixture::new();
        let mut file = plan().mods.remove(0).file;
        file.sha512 = Some(format!("{:x}", Sha512::digest(b"fixture")));
        let path = fixture.0.join("mods").join(&file.filename);
        assert!(!matches_downloaded_file(&path, &file).unwrap());
        std::fs::write(&path, b"fixture").unwrap();
        assert!(matches_downloaded_file(&path, &file).unwrap());
        std::fs::write(&path, b"changed").unwrap();
        assert!(!matches_downloaded_file(&path, &file).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
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

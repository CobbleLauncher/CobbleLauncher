// SPDX-License-Identifier: MIT
//! First installation of an exact local CurseForge archive and approved file plan.
//! Caller holds the stopped-instance lease. No pack updates or scripts are run.
use crate::modrinth::{
    ManagedMod, ModDownloads, ModProvider, Provenance, file_expectation, write_provider_provenance,
};
use crate::{InstallProgress, OperationControl};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::curseforge::{
    PackArchiveSelection, PackFileDestination, PackInstallPlan, ResolvedPackFile, RestrictedFile,
};
use cobble_content::{CurseForgePack, read_curseforge_pack_file};
use cobble_minecraft::DownloadSource;
use cobble_store::{ContentStore, SafeRelativePath, VerifiedArtifact};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};
type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-curseforge-pack.json";

#[derive(Clone, Serialize, Deserialize)]
pub struct CurseForgePackPlan {
    pub archive_sha256: String,
    pub optional_files: Vec<String>,
    pub files: PackInstallPlan,
}
pub struct CurseForgePackInspection {
    pub pack: CurseForgePack,
    pub archive_sha256: String,
}
#[derive(Serialize, Deserialize)]
struct State {
    archive_sha256: String,
    plan_sha256: String,
    complete: bool,
}

pub fn inspect_curseforge_pack(archive: &Path) -> Result<CurseForgePackInspection, Error> {
    inspect(archive, &OperationControl::new(Arc::new(|_| {})))
}
fn inspect(archive: &Path, control: &OperationControl) -> Result<CurseForgePackInspection, Error> {
    let mut input = std::fs::File::open(archive)?;
    if !input.metadata()?.is_file() || input.metadata()?.len() > 16 * 1024 * 1024 * 1024 {
        return Err("CurseForge archive must be a regular file no larger than 16 GiB".into());
    }
    let archive_sha256 = crate::mrpack::hash_reader_controlled(&mut input, control)?;
    input.seek(SeekFrom::Start(0))?;
    Ok(CurseForgePackInspection {
        pack: read_curseforge_pack_file(input)?,
        archive_sha256,
    })
}
pub fn curseforge_pack_pins(
    pack: &CurseForgePack,
    optional: &[String],
) -> Result<Vec<(u32, u32)>, Error> {
    let selected: BTreeSet<_> = optional.iter().cloned().collect();
    if selected.len() != optional.len()
        || selected.iter().any(|id| {
            !pack.files.iter().any(|file| {
                !file.required && format!("{}:{}", file.project_id, file.file_id) == id.as_str()
            })
        })
    {
        return Err("Unknown or duplicate optional CurseForge file".into());
    }
    Ok(pack
        .files
        .iter()
        .filter(|file| {
            file.required || selected.contains(&format!("{}:{}", file.project_id, file.file_id))
        })
        .map(|file| (file.project_id, file.file_id))
        .collect())
}
pub fn curseforge_pack_file_path(file: &ResolvedPackFile) -> Result<String, Error> {
    let name = SafeRelativePath::parse(&file.file.filename)?;
    let (folder, extension) = match file.destination {
        PackFileDestination::Mods => ("mods", "jar"),
        PackFileDestination::ResourcePack => ("resourcepacks", "zip"),
        PackFileDestination::ShaderPack => ("shaderpacks", "zip"),
    };
    if name.as_str().contains('/')
        || name.as_str().starts_with('.')
        || !name
            .as_str()
            .to_ascii_lowercase()
            .ends_with(&format!(".{extension}"))
    {
        return Err("Invalid CurseForge pack artifact filename/type".into());
    }
    file_expectation(&file.file)?;
    if !file.file.url.is_empty() && !cobble_content::curseforge::official_file_url(&file.file.url) {
        return Err("Unsupported CurseForge artifact origin".into());
    }
    Ok(format!("{folder}/{}", name.as_str()))
}
pub fn validate_curseforge_pack_plan(
    pack: &CurseForgePack,
    plan: &CurseForgePackPlan,
) -> Result<Vec<String>, Error> {
    if plan.archive_sha256.len() != 64
        || !plan
            .archive_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || plan.files.files.len() > 256
    {
        return Err("Invalid CurseForge pack plan".into());
    }
    let loader = pack
        .loader
        .as_ref()
        .map(|loader| loader.kind.as_str())
        .unwrap_or("minecraft");
    if !cobble_content::supported_pack_target(&pack.minecraft_version, loader) {
        return Err("Unsupported Minecraft version or CurseForge pack loader".into());
    }
    let pins = curseforge_pack_pins(pack, &plan.optional_files)?;
    let mut projects = BTreeSet::new();
    let mut identities = BTreeSet::new();
    let mut paths = Vec::new();
    for file in &plan.files.files {
        let project = number(&file.project_id)?;
        let id = number(&file.file_id)?;
        if !projects.insert(project) || !identities.insert((project, id)) {
            return Err("Duplicate CurseForge project/file selection".into());
        }
        paths.push(curseforge_pack_file_path(file)?);
    }
    if pins.iter().any(|pin| !identities.contains(pin)) {
        return Err("Resolved CurseForge files do not contain every selected manifest pin".into());
    }
    if plan.files.incompatible.iter().any(|project| {
        plan.files
            .files
            .iter()
            .any(|file| &file.project_id == project)
    }) {
        return Err("CurseForge pack selects an incompatible project".into());
    }
    let restricted: BTreeSet<_> = plan
        .files
        .restricted
        .iter()
        .map(|file| (&file.project_id, &file.file_id))
        .collect();
    if restricted.len() != plan.files.restricted.len()
        || plan.files.files.iter().any(|file| {
            file.file.url.is_empty() != restricted.contains(&(&file.project_id, &file.file_id))
        })
        || plan.files.restricted.iter().any(|item| {
            !plan.files.files.iter().any(|file| {
                file.project_id == item.project_id
                    && file.file_id == item.file_id
                    && file.file.filename == item.file_name
                    && file.file.size == item.size_bytes
            })
        })
    {
        return Err("Restricted CurseForge files do not match the resolved plan".into());
    }
    cobble_content::validate_curseforge_pack_paths(pack, &paths)?;
    Ok(paths)
}
fn number(value: &str) -> Result<u32, Error> {
    let parsed = value.parse::<u32>()?;
    if parsed == 0 || value != parsed.to_string() {
        return Err("Invalid CurseForge file identity".into());
    }
    Ok(parsed)
}
pub fn missing_curseforge_pack_manual_files(
    plan: &CurseForgePackPlan,
    key: String,
) -> Result<Vec<RestrictedFile>, Error> {
    if plan.files.restricted.is_empty() {
        return Ok(vec![]);
    }
    let downloads = ModDownloads::open_for(ModProvider::CurseForge, Some(key))?;
    plan.files
        .restricted
        .iter()
        .filter_map(|item| {
            let selected =
                plan.files.files.iter().find(|file| {
                    file.project_id == item.project_id && file.file_id == item.file_id
                });
            match selected {
                Some(file) => match downloads.contains(&file.file) {
                    Ok(true) => None,
                    Ok(false) => Some(Ok(item.clone())),
                    Err(error) => Some(Err(error)),
                },
                None => Some(Err("Unknown restricted pack file".into())),
            }
        })
        .collect()
}
pub fn import_curseforge_pack_manual_file(
    plan: &CurseForgePackPlan,
    project: &str,
    id: &str,
    input: std::fs::File,
    key: String,
) -> Result<(), Error> {
    if !plan
        .files
        .restricted
        .iter()
        .any(|item| item.project_id == project && item.file_id == id)
    {
        return Err("Manual file is not in the current CurseForge pack plan".into());
    }
    let file = plan
        .files
        .files
        .iter()
        .find(|file| file.project_id == project && file.file_id == id)
        .ok_or("Unknown manual pack file")?;
    curseforge_pack_file_path(file)?;
    ModDownloads::open_for(ModProvider::CurseForge, Some(key))?.ingest_manual(&file.file, input)
}
fn content() -> Result<ContentStore, Error> {
    let cache = dirs::cache_dir()
        .ok_or("Could not locate content cache")?
        .join("CobbleLauncher/content");
    std::fs::create_dir_all(&cache)?;
    Ok(ContentStore::open(Dir::open_ambient_dir(
        cache,
        cap_std::ambient_authority(),
    )?)?)
}
fn prepare(target: &Path, version: &str, state: &State) -> Result<Dir, Error> {
    if target.exists() {
        let root = Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
        if root.entries()?.next().is_some() {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let old: State = serde_json::from_reader(
                root.open_with(MARKER, &options)
                    .map_err(|_| "CurseForge pack installation requires an empty folder")?
                    .take(1024 * 1024),
            )?;
            if old.complete
                || old.archive_sha256 != state.archive_sha256
                || old.plan_sha256 != state.plan_sha256
            {
                return Err(
                    "Retry requires the same incomplete CurseForge archive and resolved selection"
                        .into(),
                );
            }
        }
    }
    crate::install::prepare_target(target, version, false)
}
fn write_state(root: &Dir, state: &State) -> Result<(), Error> {
    let temp = format!("{MARKER}.tmp");
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create(true)
        .truncate(true)
        .follow(FollowSymlinks::No);
    let mut file = root.open_with(&temp, &options)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    root.rename(&temp, root, MARKER)?;
    Ok(())
}
pub(super) fn ensure_complete(directory: &Path) -> Result<(), Error> {
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let input = match root.open_with(MARKER, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let state: State = serde_json::from_reader(input.take(1024 * 1024))?;
    if !state.complete {
        return Err("CurseForge pack installation is incomplete; retry before launching".into());
    }
    Ok(())
}
pub async fn install_curseforge_pack_controlled(
    archive: &Path,
    target: &Path,
    source: DownloadSource,
    plan: &CurseForgePackPlan,
    key: String,
    control: OperationControl,
) -> Result<(), Error> {
    control.check_cancelled()?;
    let inspected = inspect(archive, &control)?;
    if inspected.archive_sha256 != plan.archive_sha256 {
        return Err("Selected CurseForge archive changed since preview".into());
    }
    let pack = inspected.pack;
    let paths = validate_curseforge_pack_plan(&pack, plan)?;
    if !plan.files.restricted.is_empty()
        && !missing_curseforge_pack_manual_files(plan, key.clone())?.is_empty()
    {
        return Err("Verify every manually downloaded CurseForge file before installing".into());
    }
    let mut state = State {
        archive_sha256: plan.archive_sha256.clone(),
        plan_sha256: format!("{:x}", Sha256::digest(serde_json::to_vec(plan)?)),
        complete: false,
    };
    let root = prepare(target, &pack.minecraft_version, &state)?;
    write_state(&root, &state)?;
    let progress = control.progress();
    let base_progress = progress.clone();
    let base_control = control.with_progress(Arc::new(move |mut event| {
        event.phase = format!("pack-base-{}", event.phase);
        base_progress(event);
    }));
    match pack
        .loader
        .as_ref()
        .map(|loader| (loader.kind.as_str(), loader.version.as_str()))
    {
        Some(("fabric", loader)) => {
            crate::install_fabric_controlled(
                &pack.minecraft_version,
                loader,
                target,
                source,
                base_control,
            )
            .await?;
        }
        Some((kind @ ("forge" | "neoforge"), loader)) => {
            crate::install_forge_controlled(
                &pack.minecraft_version,
                loader,
                if kind == "forge" {
                    crate::ForgeKind::Forge
                } else {
                    crate::ForgeKind::NeoForge
                },
                target,
                source,
                base_control,
            )
            .await?;
        }
        None => {
            crate::install_game_controlled(&pack.minecraft_version, target, source, base_control)
                .await?;
        }
        _ => return Err("Unsupported CurseForge pack loader".into()),
    }
    let content = content()?;
    let downloads = if plan.files.files.is_empty() {
        None
    } else {
        Some(Arc::new(ModDownloads::open_for(
            ModProvider::CurseForge,
            Some(key),
        )?))
    };
    let mut files = BTreeMap::new();
    if let Some(downloads) = downloads {
        files = collect_pack_files(
            plan.files.files.iter().cloned().zip(paths).collect(),
            downloads.concurrency(),
            &control,
            move |selected, control| {
                let downloads = downloads.clone();
                async move {
                    downloads
                        .fetch_file_controlled(&selected.file, &control)
                        .await
                }
            },
        )
        .await?;
    }
    let mut input = std::fs::File::open(archive)?;
    if crate::mrpack::hash_reader_controlled(&mut input, &control)? != plan.archive_sha256 {
        return Err("CurseForge archive changed during installation".into());
    }
    input.seek(SeekFrom::Start(0))?;
    let mut zip = zip::ZipArchive::new(input)?;
    crate::mrpack::apply_pack_overrides(&mut zip, &pack.overrides, &content, &mut files)?;
    publish(&root, &content, &files, &pack, plan, &control)?;
    state.complete = true;
    write_state(&root, &state)?;
    progress(InstallProgress {
        phase: "complete".into(),
        completed: files.len(),
        total: files.len(),
        reused: 0,
        detail: None,
    });
    Ok(())
}
// Bounded scheduling only; production fetches still use the shared verified downloader.
async fn collect_pack_files<F, Fut>(
    jobs: Vec<(ResolvedPackFile, String)>,
    concurrency: usize,
    control: &OperationControl,
    fetch: F,
) -> Result<BTreeMap<String, VerifiedArtifact>, Error>
where
    F: Fn(ResolvedPackFile, OperationControl) -> Fut,
    Fut: std::future::Future<Output = Result<VerifiedArtifact, Error>> + Send + 'static,
{
    let total = jobs.len();
    let mut files = BTreeMap::new();
    let mut queued = jobs.into_iter();
    let mut pending = tokio::task::JoinSet::new();
    loop {
        if control.is_cancelled() {
            while pending.join_next().await.is_some() {}
            control.check_cancelled()?;
        }
        while pending.len() < concurrency {
            let Some((selected, path)) = queued.next() else {
                break;
            };
            let future = fetch(selected, control.clone());
            pending.spawn(async move { future.await.map(|artifact| (path, artifact)) });
        }
        let Some(result) = pending.join_next().await else {
            break;
        };
        match result {
            Ok(Ok((path, artifact))) => {
                files.insert(path, artifact);
            }
            failure => {
                control.cancel();
                // Let controlled downloads clean their staging before returning.
                while pending.join_next().await.is_some() {}
                return Err(match failure {
                    Ok(Err(error)) => error,
                    Err(error) => error.into(),
                    Ok(Ok(_)) => unreachable!(),
                });
            }
        }
        (control.progress())(InstallProgress {
            phase: "pack-download".into(),
            completed: files.len(),
            total,
            reused: 0,
            detail: None,
        });
    }
    Ok(files)
}

fn publish(
    root: &Dir,
    content: &ContentStore,
    files: &BTreeMap<String, VerifiedArtifact>,
    pack: &CurseForgePack,
    plan: &CurseForgePackPlan,
    control: &OperationControl,
) -> Result<(), Error> {
    crate::mrpack::publish_controlled(root, content, files, control)?;
    control.check_cancelled()?;
    // Locally overridden JARs are pack-local content, never official CF receipts.
    let mut provenance = Provenance::new();
    for selected in &plan.files.files {
        let path = curseforge_pack_file_path(selected)?;
        if selected.destination == PackFileDestination::Mods
            && !pack.overrides.iter().any(|entry| entry.path == path)
        {
            provenance.insert(
                selected.project_id.clone(),
                ManagedMod {
                    version_id: selected.file_id.clone(),
                    filename: selected.file.filename.clone(),
                    sha1: selected.file.sha1.clone(),
                },
            );
        }
    }
    write_provider_provenance(root, &provenance, ModProvider::CurseForge)
}

pub async fn download_curseforge_pack_archive_controlled(
    selection: &PackArchiveSelection,
    destination: &Path,
    key: String,
    control: OperationControl,
) -> Result<CurseForgePackInspection, Error> {
    control.check_cancelled()?;
    validate_archive(selection)?;
    if selection.restricted.is_some() || selection.file.url.is_empty() {
        return Err("This CurseForge pack archive requires a verified manual download".into());
    }
    let downloads = ModDownloads::open_for(ModProvider::CurseForge, Some(key))?;
    let artifact = downloads
        .fetch_file_controlled(&selection.file, &control)
        .await?;
    publish_archive(
        selection,
        destination,
        &downloads.content,
        &artifact,
        &control,
    )
}
pub fn import_curseforge_pack_archive(
    selection: &PackArchiveSelection,
    input: std::fs::File,
    destination: &Path,
    key: String,
) -> Result<CurseForgePackInspection, Error> {
    validate_archive(selection)?;
    let downloads = ModDownloads::open_for(ModProvider::CurseForge, Some(key))?;
    downloads.ingest_manual(&selection.file, input)?;
    let expected = file_expectation(&selection.file)?;
    let content = &downloads.content;
    // The manual ingest was verified. Read its strong artifact from the CAS via
    // the same expected-hash index, never by basename or user-provided URL.
    let cache = dirs::cache_dir()
        .ok_or("Could not locate cache")?
        .join("CobbleLauncher/sha1");
    let index = Dir::open_ambient_dir(cache, cap_std::ambient_authority())?;
    let artifact = crate::cache::lookup(content, &index, expected)?
        .ok_or("Verified manual archive disappeared")?;
    publish_archive(
        selection,
        destination,
        content,
        &artifact,
        &OperationControl::new(Arc::new(|_| {})),
    )
}
fn validate_archive(selection: &PackArchiveSelection) -> Result<(), Error> {
    let path = SafeRelativePath::parse(&selection.file.filename)?;
    file_expectation(&selection.file)?;
    if path.as_str().contains('/')
        || path.as_str().starts_with('.')
        || !path.as_str().to_ascii_lowercase().ends_with(".zip")
        || selection.restricted.is_some() != selection.file.url.is_empty()
        || (!selection.file.url.is_empty()
            && !cobble_content::curseforge::official_file_url(&selection.file.url))
        || selection.restricted.as_ref().is_some_and(|restricted| {
            restricted.file_name != selection.file.filename
                || restricted.size_bytes != selection.file.size
        })
    {
        return Err("Invalid official CurseForge pack archive selection".into());
    }
    Ok(())
}
fn publish_archive(
    selection: &PackArchiveSelection,
    destination: &Path,
    content: &ContentStore,
    artifact: &VerifiedArtifact,
    control: &OperationControl,
) -> Result<CurseForgePackInspection, Error> {
    control.check_cancelled()?;
    if destination.try_exists()? {
        if !crate::modrinth::matches_downloaded_file(destination, &selection.file)? {
            return Err("Existing CurseForge pack archive differs; it was preserved".into());
        }
        return inspect(destination, control);
    }
    let parent = destination.parent().ok_or("Pack source has no parent")?;
    let root = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
    let name = SafeRelativePath::parse(
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid pack source filename")?,
    )?;
    content.copy_to(artifact, &root, &name)?;
    match inspect(destination, control) {
        Ok(pack) => Ok(pack),
        Err(error) => {
            let _ = root.remove_file(name.as_str());
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_content::ModrinthFile;
    use cobble_content::{CurseForgePackFile, PackOverride};
    use cobble_store::{ArtifactExpectation, ExpectedDigests, IntegrityBasis};

    fn pack() -> CurseForgePack {
        CurseForgePack {
            name: "Original fixture".into(),
            version: "1".into(),
            minecraft_version: "1.21.1".into(),
            loader: None,
            files: vec![
                CurseForgePackFile {
                    project_id: 1,
                    file_id: 10,
                    required: true,
                },
                CurseForgePackFile {
                    project_id: 2,
                    file_id: 20,
                    required: false,
                },
            ],
            overrides: vec![],
        }
    }
    fn file(
        project: &str,
        id: &str,
        filename: &str,
        destination: PackFileDestination,
    ) -> ResolvedPackFile {
        ResolvedPackFile {
            project_id: project.into(),
            file_id: id.into(),
            file: ModrinthFile {
                filename: filename.into(),
                url: "https://edge.forgecdn.net/files/1/1/fixture.jar".into(),
                size: 5,
                sha1: "a".repeat(40),
                sha512: None,
            },
            destination,
            name: "Fixture".into(),
            version_number: "1".into(),
            required_by: None,
        }
    }
    fn plan() -> CurseForgePackPlan {
        CurseForgePackPlan {
            archive_sha256: "b".repeat(64),
            optional_files: vec![],
            files: PackInstallPlan {
                files: vec![file("1", "10", "fixture.jar", PackFileDestination::Mods)],
                restricted: vec![],
                incompatible: vec![],
            },
        }
    }
    #[test]
    fn exact_pins_routes_and_plan_conflicts() {
        let mut pack = pack();
        assert_eq!(curseforge_pack_pins(&pack, &[]).unwrap(), vec![(1, 10)]);
        assert_eq!(
            curseforge_pack_pins(&pack, &["2:20".into()]).unwrap(),
            vec![(1, 10), (2, 20)]
        );
        assert!(curseforge_pack_pins(&pack, &["2:21".into()]).is_err());
        assert!(curseforge_pack_pins(&pack, &["2:20".into(), "2:20".into()]).is_err());
        let mut plan = plan();
        assert_eq!(
            validate_curseforge_pack_plan(&pack, &plan).unwrap(),
            vec!["mods/fixture.jar"]
        );
        plan.files.incompatible.push("1".into());
        assert!(validate_curseforge_pack_plan(&pack, &plan).is_err());
        plan.files.incompatible.clear();
        plan.files.files[0].file_id = "11".into();
        assert!(validate_curseforge_pack_plan(&pack, &plan).is_err());
        plan.files.files[0].file_id = "10".into();
        pack.overrides.push(PackOverride {
            entry_index: 0,
            path: "mods/Fixture.jar".into(),
            layer: 0,
            size: 5,
        });
        assert!(validate_curseforge_pack_plan(&pack, &plan).is_err());
        pack.overrides[0].path = "mods/fixture.jar".into();
        assert!(validate_curseforge_pack_plan(&pack, &plan).is_ok());
        for (destination, expected) in [
            (
                PackFileDestination::ResourcePack,
                "resourcepacks/fixture.zip",
            ),
            (PackFileDestination::ShaderPack, "shaderpacks/fixture.zip"),
        ] {
            assert_eq!(
                curseforge_pack_file_path(&file("3", "30", "fixture.zip", destination)).unwrap(),
                expected
            );
        }
    }
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("cobble-cf-publication-{stamp}"));
            std::fs::create_dir_all(path.join("content")).unwrap();
            std::fs::create_dir_all(path.join("game")).unwrap();
            Self(path)
        }
        fn dir(&self, name: &str) -> Dir {
            Dir::open_ambient_dir(self.0.join(name), cap_std::ambient_authority()).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn publication_preserves_edits_and_does_not_mislabel_overrides() {
        let fixture = Fixture::new();
        let content = ContentStore::open(fixture.dir("content")).unwrap();
        let artifact = content
            .ingest(
                b"local".as_slice(),
                ArtifactExpectation {
                    expected_size: Some(5),
                    max_bytes: 5,
                    basis: IntegrityBasis::UserConfirmedLocal,
                    digests: ExpectedDigests::default(),
                },
            )
            .unwrap()
            .artifact;
        let mut files = BTreeMap::new();
        files.insert("mods/fixture.jar".into(), artifact);
        let mut pack = pack();
        pack.overrides.push(PackOverride {
            entry_index: 0,
            path: "mods/fixture.jar".into(),
            layer: 0,
            size: 5,
        });
        let control = OperationControl::new(Arc::new(|_| {}));
        let root = fixture.dir("game");
        publish(&root, &content, &files, &pack, &plan(), &control).unwrap();
        assert_eq!(
            std::fs::read(fixture.0.join("game/mods/fixture.jar")).unwrap(),
            b"local"
        );
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixture.0.join("game/.cobble-curseforge.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(receipt, serde_json::json!({}));
        publish(&root, &content, &files, &pack, &plan(), &control).unwrap();
        std::fs::write(fixture.0.join("game/mods/fixture.jar"), b"edited").unwrap();
        assert!(publish(&root, &content, &files, &pack, &plan(), &control).is_err());
        assert_eq!(
            std::fs::read(fixture.0.join("game/mods/fixture.jar")).unwrap(),
            b"edited"
        );
        control.cancel();
        assert!(publish(&root, &content, &files, &pack, &plan(), &control).is_err());
    }

    #[test]
    fn incomplete_marker_requires_exact_retry_and_blocks_launch() {
        let fixture = Fixture::new();
        let target = fixture.0.join("game");
        let mut state = State {
            archive_sha256: "a".repeat(64),
            plan_sha256: "b".repeat(64),
            complete: false,
        };
        let root = prepare(&target, "1.21.1", &state).unwrap();
        write_state(&root, &state).unwrap();
        assert!(ensure_complete(&target).is_err());
        assert!(prepare(&target, "1.21.1", &state).is_ok());
        state.plan_sha256 = "c".repeat(64);
        assert!(prepare(&target, "1.21.1", &state).is_err());
        state.plan_sha256 = "b".repeat(64);
        state.complete = true;
        write_state(&root, &state).unwrap();
        ensure_complete(&target).unwrap();
        assert!(prepare(&target, "1.21.1", &state).is_err());
    }

    #[test]
    fn original_archive_is_pinned_and_overrides_use_verified_cas() {
        let fixture = Fixture::new();
        let archive = fixture.0.join("original.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("manifest.json", options).unwrap();
        writer.write_all(br#"{"manifestType":"minecraftModpack","manifestVersion":1,"name":"Original fixture","version":"1","minecraft":{"version":"1.21.1","modLoaders":[]},"files":[],"overrides":"overrides"}"#).unwrap();
        writer
            .start_file("overrides/config/original.txt", options)
            .unwrap();
        writer.write_all(b"original bytes").unwrap();
        writer.finish().unwrap();
        let before = std::fs::read(&archive).unwrap();
        let inspection = inspect_curseforge_pack(&archive).unwrap();
        assert_eq!(
            inspection.archive_sha256,
            format!("{:x}", Sha256::digest(&before))
        );
        let content = ContentStore::open(fixture.dir("content")).unwrap();
        let mut files = BTreeMap::new();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).unwrap()).unwrap();
        crate::mrpack::apply_pack_overrides(
            &mut zip,
            &inspection.pack.overrides,
            &content,
            &mut files,
        )
        .unwrap();
        let plan = CurseForgePackPlan {
            archive_sha256: inspection.archive_sha256,
            optional_files: vec![],
            files: PackInstallPlan {
                files: vec![],
                restricted: vec![],
                incompatible: vec![],
            },
        };
        validate_curseforge_pack_plan(&inspection.pack, &plan).unwrap();
        publish(
            &fixture.dir("game"),
            &content,
            &files,
            &inspection.pack,
            &plan,
            &OperationControl::new(Arc::new(|_| {})),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(fixture.0.join("game/config/original.txt")).unwrap(),
            b"original bytes"
        );
        assert_eq!(std::fs::read(&archive).unwrap(), before);
    }

    #[tokio::test]
    async fn bounded_jobs_overlap_and_publish_every_result() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let fixture = Fixture::new();
        let content = Arc::new(ContentStore::open(fixture.dir("content")).unwrap());
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let active = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let observed = maximum.clone();
        let jobs = (1..=4)
            .map(|n| {
                (
                    file(
                        &n.to_string(),
                        "10",
                        &format!("{n}.jar"),
                        PackFileDestination::Mods,
                    ),
                    format!("mods/{n}.jar"),
                )
            })
            .collect();
        let control = OperationControl::new(Arc::new(|_| {}));
        let files = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            collect_pack_files(jobs, 2, &control, move |_, _| {
                let (content, barrier, active, maximum) = (
                    content.clone(),
                    barrier.clone(),
                    active.clone(),
                    maximum.clone(),
                );
                async move {
                    let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                    maximum.fetch_max(current, Ordering::SeqCst);
                    barrier.wait().await;
                    let artifact = content
                        .ingest(
                            b"local".as_slice(),
                            ArtifactExpectation {
                                expected_size: Some(5),
                                max_bytes: 5,
                                basis: IntegrityBasis::UserConfirmedLocal,
                                digests: ExpectedDigests::default(),
                            },
                        )?
                        .artifact;
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(artifact)
                }
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(observed.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn manual_file_requires_exact_official_hash_and_size() {
        use sha1::Sha1;
        let fixture = Fixture::new();
        let downloads = ModDownloads::open_at(
            &fixture.0.join("manual-cache"),
            ModProvider::CurseForge,
            Some("fixture-not-a-token".into()),
        )
        .unwrap();
        let mut selected = file("1", "10", "fixture.jar", PackFileDestination::Mods);
        selected.file.url.clear();
        selected.file.sha1 = format!("{:x}", Sha1::digest(b"local"));
        let path = fixture.0.join("manual.jar");
        std::fs::write(&path, b"wrong").unwrap();
        assert!(
            downloads
                .ingest_manual(&selected.file, std::fs::File::open(&path).unwrap())
                .is_err()
        );
        assert!(!downloads.contains(&selected.file).unwrap());
        std::fs::write(&path, b"local").unwrap();
        downloads
            .ingest_manual(&selected.file, std::fs::File::open(&path).unwrap())
            .unwrap();
        assert!(downloads.contains(&selected.file).unwrap());
    }

    #[tokio::test]
    #[ignore = "Explicit runtime CF credential; bounded official archive and metadata only, no game install"]
    async fn official_small_pack_archive_preview_and_exact_resolution() {
        use cobble_content::curseforge::{ContentKind, CurseForgeClient, DownloadAvailability};
        let key = std::env::var("COURSE_FORGE_API").expect("explicit runtime credential required");
        let client = CurseForgeClient::new(key.clone()).unwrap();
        let projects = client
            .search(
                "Fabulously Optimized",
                ContentKind::Modpack,
                Some("1.21.1"),
                Some("fabric"),
                0,
            )
            .await
            .unwrap();
        let project = projects
            .hits
            .iter()
            .find(|project| project.title.eq_ignore_ascii_case("Fabulously Optimized"))
            .expect("official representative pack not found");
        let project_id = project.project_id.parse().unwrap();
        let listing = client
            .files(project_id, Some("1.21.1"), Some("fabric"), 0)
            .await
            .unwrap();
        let listed = listing
            .files
            .iter()
            .find(|file| {
                file.download_availability == DownloadAvailability::Available
                    && file.size_bytes <= 4 * 1024 * 1024
            })
            .expect("no representative archive within four MiB budget");
        let file_id = listed.file_id.parse().unwrap();
        let selected = client
            .resolve_pack_archive(project_id, file_id)
            .await
            .unwrap();
        assert!(selected.file.size <= 4 * 1024 * 1024);
        let fixture = Fixture::new();
        let downloads = ModDownloads::open_at(
            &fixture.0.join("official-cache"),
            ModProvider::CurseForge,
            Some(key),
        )
        .unwrap();
        assert!(!downloads.contains(&selected.file).unwrap());
        let control = OperationControl::new(Arc::new(|_| {}));
        let retained = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/cf-pack-live");
        std::fs::create_dir_all(&retained).unwrap();
        let retained_archive = retained.join("original.zip");
        if retained_archive.exists()
            && crate::modrinth::matches_downloaded_file(&retained_archive, &selected.file).unwrap()
        {
            downloads
                .ingest_manual(
                    &selected.file,
                    std::fs::File::open(&retained_archive).unwrap(),
                )
                .unwrap();
        }
        let artifact = downloads
            .fetch_file_controlled(&selected.file, &control)
            .await
            .unwrap();
        let archive = fixture.0.join("original.zip");
        let inspection =
            publish_archive(&selected, &archive, &downloads.content, &artifact, &control).unwrap();
        let original = std::fs::read(&archive).unwrap();
        std::fs::write(&retained_archive, &original).unwrap();
        std::fs::write(retained.join("origin.json"),serde_json::to_vec_pretty(&serde_json::json!({"project_id":project_id,"file_id":file_id,"archive_sha256":inspection.archive_sha256})).unwrap()).unwrap();
        let pins = curseforge_pack_pins(&inspection.pack, &[]).unwrap();
        assert!(
            !pins.is_empty() && pins.len() <= 64,
            "representative pack outside metadata smoke budget"
        );
        println!(
            "official archive project={project_id} file={file_id} bytes={} minecraft={} loader={:?} manifest_pins={} sha256={}",
            original.len(),
            inspection.pack.minecraft_version,
            inspection
                .pack
                .loader
                .as_ref()
                .map(|loader| loader.kind.as_str()),
            pins.len(),
            inspection.archive_sha256
        );
        let files = client
            .resolve_pack_files(
                &pins,
                &inspection.pack.minecraft_version,
                inspection
                    .pack
                    .loader
                    .as_ref()
                    .map(|loader| loader.kind.as_str()),
            )
            .await
            .unwrap();
        let plan = CurseForgePackPlan {
            archive_sha256: inspection.archive_sha256.clone(),
            optional_files: vec![],
            files,
        };
        validate_curseforge_pack_plan(&inspection.pack, &plan).unwrap();
        assert_eq!(
            inspect_curseforge_pack(&archive).unwrap().archive_sha256,
            inspection.archive_sha256
        );
        assert_eq!(std::fs::read(&archive).unwrap(), original);
        println!(
            "official pack project={project_id} file={file_id} archive_bytes={} manifest_pins={} resolved_files={} restricted={} overrides={} archive_sha256={}",
            original.len(),
            pins.len(),
            plan.files.files.len(),
            plan.files.restricted.len(),
            inspection.pack.overrides.len(),
            inspection.archive_sha256
        );
    }
}

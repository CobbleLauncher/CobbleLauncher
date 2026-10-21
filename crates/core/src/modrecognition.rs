// SPDX-License-Identifier: MIT
//! Explicit, hash-based adoption of local Mods into existing Modrinth tracking.
//! Callers hold the stopped-instance content lease. No JAR or config is changed.
use crate::modrinth::{ManagedMod, Provenance, read_provenance, write_provenance};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cap_std::time::SystemTime;
use cobble_content::{IdentifiedMod, identify_modrinth_file_with_broker};
use cobble_net::{Broker, BrokerConfig};
use cobble_store::SafeRelativePath;
use serde::Serialize;
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    io::{self, Read},
    path::{Path, PathBuf},
};
use tokio::task::JoinSet;

type Error = Box<dyn std::error::Error + Send + Sync>;
const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ModRecognitionOutcome {
    pub recognized: usize,
    pub unmatched: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileState {
    name: String,
    size: u64,
    modified: Option<SystemTime>,
}

#[derive(Clone, Debug)]
struct LocalFile {
    state: FileState,
    canonical_name: String,
    hash: Option<String>,
    managed: bool,
}

struct Snapshot {
    directory: PathBuf,
    provenance: Provenance,
    files: Vec<LocalFile>,
    present_projects: BTreeSet<String>,
}

fn open_root(path: &Path) -> Result<Dir, Error> {
    Ok(Dir::open_ambient_dir(path, cap_std::ambient_authority())?)
}

fn local_files(root: &Dir) -> Result<Vec<FileState>, Error> {
    let mods = match root.open_dir_nofollow("mods") {
        Ok(mods) => mods,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut files = Vec::new();
    for entry in mods.entries()? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".jar") && !lower.ends_with(".jar.disabled") {
            continue;
        }
        let metadata = mods.symlink_metadata(&name)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            continue;
        }
        // Opening without following links also rejects Windows file reparse
        // redirects before bytes are hashed; directory links never get scanned.
        files.push(FileState {
            name,
            size: metadata.len(),
            modified: metadata.modified().ok(),
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

fn canonical_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".disabled") {
        name[..name.len() - ".disabled".len()].to_owned()
    } else {
        name.to_owned()
    }
}

fn snapshot(directory: PathBuf) -> Result<Snapshot, Error> {
    let root = open_root(&directory)?;
    let provenance = read_provenance(&root)?;
    let states = local_files(&root)?;
    let mut aliases = BTreeMap::<String, usize>::new();
    for state in &states {
        *aliases
            .entry(canonical_name(&state.name).to_ascii_lowercase())
            .or_default() += 1;
    }
    let mods = if states.is_empty() {
        None
    } else {
        Some(root.open_dir_nofollow("mods")?)
    };
    let mut files = Vec::new();
    let mut present_projects = BTreeSet::new();
    for state in states {
        let canonical_name = canonical_name(&state.name);
        let managed_projects: Vec<_> = provenance
            .iter()
            .filter(|(_, entry)| entry.filename.eq_ignore_ascii_case(&canonical_name))
            .map(|(id, _)| id.clone())
            .collect();
        let managed = !managed_projects.is_empty();
        present_projects.extend(managed_projects);
        let safe = SafeRelativePath::parse(&canonical_name).is_ok()
            && !canonical_name.contains(['/', '\\'])
            && !canonical_name.starts_with('.')
            && (state.name == canonical_name || state.name == format!("{canonical_name}.disabled"));
        let hash = if managed
            || !safe
            || state.size == 0
            || state.size > MAX_MOD_BYTES
            || aliases[&canonical_name.to_ascii_lowercase()] != 1
        {
            None
        } else {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let mut file = mods
                .as_ref()
                .ok_or("Missing Mods directory")?
                .open_with(&state.name, &options)?;
            let metadata = file.metadata()?;
            if !metadata.is_file()
                || metadata.len() != state.size
                || metadata.modified().ok() != state.modified
            {
                return Err("Local Mod changed during recognition; retry".into());
            }
            let mut hash = Sha1::new();
            let mut buffer = [0; 65536];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            Some(format!("{:x}", hash.finalize()))
        };
        files.push(LocalFile {
            state,
            canonical_name,
            hash,
            managed,
        });
    }
    Ok(Snapshot {
        directory,
        provenance,
        files,
        present_projects,
    })
}

pub async fn recognize_modrinth_mods(
    directory: &Path,
    mc: &str,
    loader: &str,
) -> Result<ModRecognitionOutcome, Error> {
    let broker = Broker::new(BrokerConfig::default())?;
    recognize_with(directory, mc, loader, move |hash, mc, loader| {
        let broker = broker.clone();
        async move { identify_modrinth_file_with_broker(&broker, &hash, &mc, &loader).await }
    })
    .await
}

async fn recognize_with<F, Fut>(
    directory: &Path,
    mc: &str,
    loader: &str,
    lookup: F,
) -> Result<ModRecognitionOutcome, Error>
where
    F: Fn(String, String, String) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Result<Option<IdentifiedMod>, Error>> + Send + 'static,
{
    let directory = directory.to_owned();
    let snapshot = tokio::task::spawn_blocking(move || snapshot(directory)).await??;
    let mut pending = snapshot
        .files
        .iter()
        .filter(|file| file.hash.is_some())
        .cloned();
    let mut tasks = JoinSet::new();
    let spawn = |tasks: &mut JoinSet<_>, file: LocalFile| {
        let future = lookup.clone()(
            file.hash.clone().unwrap_or_default(),
            mc.to_owned(),
            loader.to_owned(),
        );
        tasks.spawn(async move { Ok::<_, Error>((file, future.await?)) });
    };
    for _ in 0..4 {
        if let Some(file) = pending.next() {
            spawn(&mut tasks, file);
        }
    }
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        results.push(result??);
        if let Some(file) = pending.next() {
            spawn(&mut tasks, file);
        }
    }
    tokio::task::spawn_blocking(move || commit(snapshot, results)).await?
}

fn commit(
    snapshot: Snapshot,
    results: Vec<(LocalFile, Option<IdentifiedMod>)>,
) -> Result<ModRecognitionOutcome, Error> {
    let root = open_root(&snapshot.directory)?;
    if read_provenance(&root)? != snapshot.provenance
        || local_files(&root)?
            != snapshot
                .files
                .iter()
                .map(|file| file.state.clone())
                .collect::<Vec<_>>()
    {
        return Err("Local Mods changed during recognition; retry".into());
    }
    let mut matches = BTreeMap::<String, Vec<(LocalFile, IdentifiedMod)>>::new();
    for (file, found) in results {
        if let Some(found) = found {
            // The content boundary verifies this too. Keep the association with
            // this exact snapshot even when tests or future adapters supply it.
            if file.hash.as_deref() != Some(found.sha1.as_str()) {
                return Err("Hash identification does not match the local Mod snapshot".into());
            }
            matches
                .entry(found.project_id.clone())
                .or_default()
                .push((file, found));
        }
    }
    let mut provenance = snapshot.provenance.clone();
    let mut recognized = 0;
    for (project, mut files) in matches {
        if files.len() != 1 || snapshot.present_projects.contains(&project) {
            continue;
        }
        let (file, identified) = files.pop().ok_or("Missing recognized Mod")?;
        provenance.insert(
            project,
            ManagedMod {
                version_id: identified.version_id,
                filename: file.canonical_name,
                sha1: identified.sha1,
            },
        );
        recognized += 1;
    }
    let unmatched = snapshot.files.iter().filter(|file| !file.managed).count() - recognized;
    if recognized > 0 {
        write_provenance(&root, &provenance)?;
    }
    Ok(ModRecognitionOutcome {
        recognized,
        unmatched,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-recognition-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(path.join("mods")).unwrap();
            Self(path)
        }
        fn root(&self) -> Dir {
            open_root(&self.0).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn identified(hash: String, project: &str) -> IdentifiedMod {
        IdentifiedMod {
            project_id: project.into(),
            version_id: "known-version".into(),
            sha1: hash,
        }
    }

    #[tokio::test]
    async fn renamed_disabled_recognition_keeps_bytes_and_is_idempotent() {
        let fixture = Fixture::new();
        let root = fixture.root();
        root.write("mods/My renamed mod.jar.disabled", b"known")
            .unwrap();
        root.write("mods/local.jar", b"unknown").unwrap();
        root.create_dir_all("config").unwrap();
        root.write("config/keep.txt", b"settings").unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let lookup = move |hash: String, _: String, _: String| {
            count.fetch_add(1, Ordering::Relaxed);
            async move {
                Ok(if hash == format!("{:x}", Sha1::digest(b"known")) {
                    Some(identified(hash, "Known"))
                } else {
                    None
                })
            }
        };
        let result = recognize_with(&fixture.0, "1.21.1", "fabric", lookup.clone())
            .await
            .unwrap();
        assert_eq!(
            result,
            ModRecognitionOutcome {
                recognized: 1,
                unmatched: 1
            }
        );
        let provenance = read_provenance(&root).unwrap();
        assert_eq!(provenance["Known"].filename, "My renamed mod.jar");
        assert_eq!(
            root.read("mods/My renamed mod.jar.disabled").unwrap(),
            b"known"
        );
        assert_eq!(root.read("config/keep.txt").unwrap(), b"settings");
        assert_eq!(
            recognize_with(&fixture.0, "1.21.1", "fabric", lookup)
                .await
                .unwrap(),
            ModRecognitionOutcome {
                recognized: 0,
                unmatched: 1
            }
        );
        assert_eq!(calls.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn duplicate_project_files_are_skipped_and_present_tracking_is_never_overwritten() {
        let fixture = Fixture::new();
        let root = fixture.root();
        root.write("mods/a.jar", b"one").unwrap();
        root.write("mods/b.jar.disabled", b"two").unwrap();
        let lookup =
            |hash: String, _: String, _: String| async move { Ok(Some(identified(hash, "Same"))) };
        assert_eq!(
            recognize_with(&fixture.0, "1.21.1", "fabric", lookup)
                .await
                .unwrap(),
            ModRecognitionOutcome {
                recognized: 0,
                unmatched: 2
            }
        );
        assert!(read_provenance(&root).unwrap().is_empty());
        let provenance = Provenance::from([(
            "Same".into(),
            ManagedMod {
                version_id: "old".into(),
                filename: "a.jar".into(),
                sha1: format!("{:x}", Sha1::digest(b"one")),
            },
        )]);
        write_provenance(&root, &provenance).unwrap();
        assert_eq!(
            recognize_with(&fixture.0, "1.21.1", "fabric", lookup)
                .await
                .unwrap(),
            ModRecognitionOutcome {
                recognized: 0,
                unmatched: 1
            }
        );
        assert_eq!(read_provenance(&root).unwrap(), provenance);
    }

    #[tokio::test]
    async fn network_error_never_saves_partial_recognition_and_final_snapshot_rejects_edits() {
        let fixture = Fixture::new();
        let root = fixture.root();
        root.write("mods/a.jar", b"one").unwrap();
        root.write("mods/b.jar", b"two").unwrap();
        let old = Provenance::new();
        write_provenance(&root, &old).unwrap();
        let error = recognize_with(
            &fixture.0,
            "1.21.1",
            "fabric",
            |hash: String, _: String, _: String| async move {
                if hash == format!("{:x}", Sha1::digest(b"two")) {
                    Err("synthetic network failure".into())
                } else {
                    Ok(Some(identified(hash, "Known")))
                }
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("network failure"));
        assert_eq!(read_provenance(&root).unwrap(), old);
        let directory = fixture.0.clone();
        let result = recognize_with(
            &fixture.0,
            "1.21.1",
            "fabric",
            move |hash: String, _: String, _: String| {
                let directory = directory.clone();
                async move {
                    std::fs::write(
                        directory.join("mods/a.jar"),
                        b"changed during network lookup",
                    )
                    .unwrap();
                    Ok(Some(identified(hash, "Known")))
                }
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(read_provenance(&root).unwrap(), old);
    }

    #[tokio::test]
    async fn more_than_128_local_mods_are_processed_with_at_most_four_lookups_in_flight() {
        let fixture = Fixture::new();
        let root = fixture.root();
        for index in 0..140 {
            root.write(
                format!("mods/mod-{index}.jar"),
                format!("mod{index}").as_bytes(),
            )
            .unwrap();
        }
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let count = Arc::new(AtomicUsize::new(0));
        let (a, p, c) = (active.clone(), peak.clone(), count.clone());
        let result = recognize_with(
            &fixture.0,
            "1.21.1",
            "fabric",
            move |_: String, _: String, _: String| {
                let (a, p, c) = (a.clone(), p.clone(), c.clone());
                async move {
                    let current = a.fetch_add(1, Ordering::SeqCst) + 1;
                    p.fetch_max(current, Ordering::SeqCst);
                    c.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    a.fetch_sub(1, Ordering::SeqCst);
                    Ok(None)
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(
            result,
            ModRecognitionOutcome {
                recognized: 0,
                unmatched: 140
            }
        );
        assert_eq!(count.load(Ordering::SeqCst), 140);
        assert!(peak.load(Ordering::SeqCst) <= 4);
        assert!(peak.load(Ordering::SeqCst) > 1);
    }
}

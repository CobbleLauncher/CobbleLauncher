// SPDX-License-Identifier: MIT
//! Local world snapshots backed by rustic's encrypted, deduplicated repository.
//! Caller owns the stopped-game/source lock and serializes repository operations.
//! Repository and restore-stage parents must be private, host-controlled directories.

mod backend;
mod insights;
pub use insights::{
    RepositoryInsights, SnapshotChange, SnapshotChangeKind, SnapshotComparePage, SnapshotComparison,
};
mod control;
mod retention;
mod source;
pub use control::{BackupControl, BackupPhase, BackupProgress};
pub use retention::{RetentionPlan, RetentionPolicy, RetentionPreview, RetentionResult};

use rustic_backend::LocalBackend;
use rustic_core::{
    BackupOptions, ConfigOptions, Credentials, KeyOptions, LocalDestination, LsOptions, OpenStatus,
    Repository, RepositoryBackends, RepositoryOptions, RestoreOptions, SnapshotOptions,
    repofile::{MasterKey, SnapshotFile},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt, fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

const COMPLETE_TAG: &str = "cobble-world-complete-v1";
const RETENTION_TAG: &str = "cobble-world-retention-v1";
const AUTOMATIC_TAG: &str = "cobble-world-automatic-v1";
const NAMED_TAG: &str = "cobble-world-named-v1";

#[derive(Debug)]
pub enum BackupError {
    InvalidInput,
    DestinationExists,
    UnsafeSource,
    IncompleteSnapshot,
    SnapshotNotFound,
    Cancelled,
    Io(std::io::Error),
    /// Fixed operation context; upstream errors are deliberately not exposed as credentials.
    Engine(&'static str),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => f.write_str("Invalid backup input"),
            Self::DestinationExists => f.write_str("Restore stage must not exist"),
            Self::UnsafeSource => f.write_str("World contains unsafe or changed entries"),
            Self::IncompleteSnapshot => f.write_str("Snapshot did not include every source entry"),
            Self::SnapshotNotFound => f.write_str("Snapshot does not belong to this world"),
            Self::Cancelled => f.write_str("Backup operation cancelled"),
            Self::Io(error) => write!(f, "Backup filesystem error: {error}"),
            Self::Engine(operation) => write!(f, "Backup engine failed to {operation}"),
        }
    }
}
impl std::error::Error for BackupError {}
impl From<std::io::Error> for BackupError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SnapshotInfo {
    pub id: String,
    pub world_id: String,
    pub name: String,
    pub created_at: String,
    /// Logical source-file bytes, not necessarily physical bytes read from disk.
    pub processed_bytes: u64,
    /// New packed content/tree bytes; excludes snapshot/index/key overhead.
    pub added_bytes: u64,
    /// Named snapshots and legacy versions are protected by the default policy.
    pub protected: bool,
    pub automatic: bool,
}

/// Open credentials are held inside rustic; this wrapper has no Debug implementation.
pub struct WorldRepository {
    root: PathBuf,
    key: MasterKey,
}

impl WorldRepository {
    fn repository(&self) -> Result<Repository<OpenStatus>, BackupError> {
        self.repository_controlled(&BackupControl::default())
    }
    fn repository_controlled(
        &self,
        control: &BackupControl,
    ) -> Result<Repository<OpenStatus>, BackupError> {
        control.check()?;
        unopened_controlled(&self.root, control)?
            .open(&Credentials::Masterkey(self.key.clone()))
            .map_err(|_| control.error("open repository"))
    }
    /// Initialize only a new, empty repository directory. Never resets an existing repository.
    pub fn create(root: &Path, password: &str) -> Result<Self, BackupError> {
        if password.is_empty() || !root.is_absolute() {
            return Err(BackupError::InvalidInput);
        }
        match fs::symlink_metadata(root) {
            Ok(metadata) => {
                if linked(&metadata) || !metadata.is_dir() || fs::read_dir(root)?.next().is_some() {
                    return Err(BackupError::InvalidInput);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(root)?;
            }
            Err(error) => return Err(error.into()),
        }
        let root = plain_dir(root)?;
        let repository = unopened(&root)?
            .init(
                &Credentials::password(password),
                &KeyOptions::default(),
                &ConfigOptions::default(),
            )
            .map_err(|_| BackupError::Engine("initialize repository"))?;
        Ok(Self {
            root,
            key: repository.key(),
        })
    }

    pub fn open(root: &Path, password: &str) -> Result<Self, BackupError> {
        if password.is_empty() || !root.is_absolute() {
            return Err(BackupError::InvalidInput);
        }
        let root = plain_dir(root)?;
        let repository = unopened(&root)?
            .open(&Credentials::password(password))
            .map_err(|_| BackupError::Engine("open repository"))?;
        Ok(Self {
            root,
            key: repository.key(),
        })
    }

    pub fn snapshot(
        &mut self,
        world: &Path,
        world_id: &str,
        name: &str,
    ) -> Result<SnapshotInfo, BackupError> {
        self.snapshot_controlled(world, world_id, name, &BackupControl::default())
    }

    pub fn snapshot_controlled(
        &mut self,
        world: &Path,
        world_id: &str,
        name: &str,
        control: &BackupControl,
    ) -> Result<SnapshotInfo, BackupError> {
        self.snapshot_kind(world, world_id, name, control, false)?
            .ok_or(BackupError::IncompleteSnapshot)
    }
    pub fn snapshot_automatic_controlled(
        &mut self,
        world: &Path,
        world_id: &str,
        control: &BackupControl,
    ) -> Result<Option<SnapshotInfo>, BackupError> {
        self.snapshot_kind(world, world_id, "", control, true)
    }
    fn snapshot_kind(
        &mut self,
        world: &Path,
        world_id: &str,
        name: &str,
        control: &BackupControl,
        automatic: bool,
    ) -> Result<Option<SnapshotInfo>, BackupError> {
        control.check()?;
        identity(world_id)?;
        validate_name(name)?;
        let world = plain_dir(world)?;
        if world.starts_with(&self.root) || self.root.starts_with(&world) {
            return Err(BackupError::InvalidInput);
        }
        control.phase(BackupPhase::Scan, None);
        let source = source::WorldSource::new_controlled(&world, control)?;
        let expected = source.manifest();
        let total = expected.values().map(|(_, size)| *size).sum();
        control.phase(BackupPhase::Snapshot, Some(total));
        let repo = self
            .repository_controlled(control)?
            .to_indexed_ids()
            .map_err(|_| control.error("load index"))?;
        let named = !name.trim().is_empty();
        let name = if !named {
            rustic_core::jiff::Timestamp::now().to_string()
        } else {
            name.trim().to_owned()
        };
        let snapshot = SnapshotOptions::default()
            .label(world_id.to_owned())
            .description(name)
            .host("cobble-local".to_owned())
            .command("cobble world snapshot".to_owned())
            .to_snapshot()
            .map_err(|_| BackupError::Engine("prepare snapshot"))?;
        let mut options = BackupOptions::default()
            .as_path(PathBuf::from("world"))
            .no_scan(true);
        // Ignore incomplete snapshots left by a prior process interruption.
        match self.list(world_id)?.first() {
            Some(parent) => options.parent_opts.parents.push(parent.id.clone()),
            None => options.parent_opts.force = true,
        }
        let saved = repo.archive(&options, &source, snapshot, &[world]);

        let saved = saved.map_err(|_| control.error("create snapshot"))?;
        control.phase(BackupPhase::Verify, None);
        // rustic deliberately skips per-file errors. Verify complete tree metadata and our
        // reader error flag before reporting success; never surface a partial backup as good.
        let complete = (|| {
            if source.failed() {
                return Ok(false);
            }
            let indexed = self
                .repository_controlled(control)?
                .to_indexed()
                .map_err(|_| control.error("verify snapshot"))?;
            let node = indexed
                .node_from_snapshot_and_path(&saved, "world")
                .map_err(|_| BackupError::Engine("verify snapshot"))?;
            let actual = manifest(
                indexed
                    .ls(&node, &LsOptions::default())
                    .map_err(|_| BackupError::Engine("verify snapshot"))?,
                control,
            )?;

            Ok(actual == expected)
        })();
        if !matches!(complete, Ok(true)) || control.is_cancelled() {
            self.repository()?
                .delete_snapshots(&[saved.id])
                .map_err(|_| BackupError::Engine("discard incomplete snapshot"))?;
            return Err(if control.is_cancelled() {
                BackupError::Cancelled
            } else {
                complete.err().unwrap_or(BackupError::IncompleteSnapshot)
            });
        }
        if automatic {
            let mut previous = self
                .repository()?
                .get_matching_snapshots(|snapshot| {
                    snapshot.label == world_id
                        && is_complete(snapshot)
                        && snapshot.tags.iter().any(|tag| tag == AUTOMATIC_TAG)
                })
                .map_err(|_| BackupError::Engine("compare automatic snapshot"))?;
            previous.sort_by(|a, b| b.time.cmp(&a.time));
            if previous
                .first()
                .is_some_and(|snapshot| snapshot.tree == saved.tree)
            {
                self.repository()?
                    .delete_snapshots(&[saved.id])
                    .map_err(|_| BackupError::Engine("discard unchanged snapshot"))?;
                return Ok(None);
            }
        }
        control.phase(BackupPhase::Publish, None);
        if control.is_cancelled() {
            self.repository()?
                .delete_snapshots(&[saved.id])
                .map_err(|_| BackupError::Engine("discard cancelled snapshot"))?;
            return Err(BackupError::Cancelled);
        }
        // Final cancellation boundary. Once this short commit starts, finish it and
        // return success even if a cancellation request arrives during publication.
        // Publishing a complete tag creates a new snapshot record only after validation.
        // A crash before this write leaves an invisible pending snapshot, never a good version.
        let pending_id = saved.id;
        let mut completed = saved;
        completed.original = Some(pending_id);
        completed.tags = format!(
            "{COMPLETE_TAG},{RETENTION_TAG}{}{}",
            if automatic {
                format!(",{AUTOMATIC_TAG}")
            } else {
                String::new()
            },
            if named {
                format!(",{NAMED_TAG}")
            } else {
                String::new()
            }
        )
        .parse()
        .map_err(|_| BackupError::Engine("tag snapshot"))?;
        let repository = self.repository()?;
        repository
            .save_snapshots(vec![completed])
            .map_err(|_| BackupError::Engine("publish complete snapshot"))?;
        let completed = repository
            .get_matching_snapshots(|snapshot| {
                snapshot.original == Some(pending_id) && is_complete(snapshot)
            })
            .map_err(|_| BackupError::Engine("read complete snapshot"))?
            .pop()
            .ok_or(BackupError::IncompleteSnapshot)?;
        repository
            .delete_snapshots(&[pending_id])
            .map_err(|_| BackupError::Engine("discard pending snapshot"))?;
        control.complete(total);
        Ok(Some(info(completed)))
    }

    pub fn list(&self, world_id: &str) -> Result<Vec<SnapshotInfo>, BackupError> {
        identity(world_id)?;
        let mut snapshots: Vec<_> = self
            .repository()?
            .get_all_snapshots()
            .map_err(|_| BackupError::Engine("list snapshots"))?
            .into_iter()
            .filter(|snapshot| snapshot.label == world_id && is_complete(snapshot))
            .map(info)
            .collect();
        snapshots.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(snapshots)
    }

    /// Materializes independent files in a fresh stage. Never writes into an existing world.
    /// On failure the owned partial stage remains for caller cleanup/recovery.
    pub fn restore(
        &mut self,
        snapshot_id: &str,
        world_id: &str,
        new_stage: &Path,
    ) -> Result<(), BackupError> {
        self.restore_controlled(snapshot_id, world_id, new_stage, &BackupControl::default())
    }

    pub fn restore_controlled(
        &mut self,
        snapshot_id: &str,
        world_id: &str,
        new_stage: &Path,
        control: &BackupControl,
    ) -> Result<(), BackupError> {
        control.check()?;
        identity(world_id)?;
        if snapshot_id.len() != 64
            || !snapshot_id.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !new_stage.is_absolute()
        {
            return Err(BackupError::InvalidInput);
        }
        match fs::symlink_metadata(new_stage) {
            Ok(_) => return Err(BackupError::DestinationExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let parent = plain_dir(new_stage.parent().ok_or(BackupError::InvalidInput)?)?;
        if parent.starts_with(&self.root) {
            return Err(BackupError::InvalidInput);
        }
        let destination = parent.join(new_stage.file_name().ok_or(BackupError::InvalidInput)?);
        control.phase(BackupPhase::Restore, None);
        let snapshot = self
            .repository_controlled(control)?
            .get_snapshot_from_str(snapshot_id, |_| true)
            .map_err(|_| {
                if control.is_cancelled() {
                    BackupError::Cancelled
                } else {
                    BackupError::SnapshotNotFound
                }
            })?;
        if snapshot.label != world_id || !is_complete(&snapshot) {
            return Err(BackupError::SnapshotNotFound);
        }
        let repo = self
            .repository_controlled(control)?
            .to_indexed()
            .map_err(|_| control.error("load restore index"))?;
        let node = repo
            .node_from_snapshot_and_path(&snapshot, "world")
            .map_err(|_| control.error("read snapshot"))?;
        let stream = repo
            .ls(&node, &LsOptions::default())
            .map_err(|_| control.error("read snapshot"))?;
        // Reject links, special nodes and unsafe paths before the engine writes anything.
        let expected = manifest(stream.clone(), control)?;
        control.check()?;
        fs::create_dir(&destination)?;
        let dest = LocalDestination::new(
            destination.to_str().ok_or(BackupError::InvalidInput)?,
            false,
            false,
        )
        .map_err(|_| BackupError::Engine("open restore stage"))?;
        let options = RestoreOptions::default().no_ownership(true);
        let plan = repo
            .prepare_restore(&options, stream.clone(), &dest, false)
            .map_err(|_| control.error("prepare restore"))?;
        control.check()?;
        repo.restore(plan, &options, stream, &dest)
            .map_err(|_| control.error("restore snapshot"))?;
        control.check()?;

        control.phase(BackupPhase::Verify, None);
        if source::WorldSource::new_controlled(&destination, control)?.manifest() != expected {
            return Err(BackupError::IncompleteSnapshot);
        }
        control.check()?;
        control.complete(expected.values().map(|(_, size)| *size).sum());
        Ok(())
    }
}

fn is_complete(snapshot: &SnapshotFile) -> bool {
    snapshot.tags.iter().any(|tag| tag == COMPLETE_TAG)
}

fn unopened(root: &Path) -> Result<Repository<()>, BackupError> {
    unopened_controlled(root, &BackupControl::default())
}

fn unopened_controlled(
    root: &Path,
    control: &BackupControl,
) -> Result<Repository<()>, BackupError> {
    let backend = LocalBackend::new(
        root.to_str().ok_or(BackupError::InvalidInput)?,
        std::iter::empty(),
    )
    .map_err(|_| BackupError::Engine("configure local repository"))?;
    let backend = backend::CheckedBackend {
        local: backend,
        control: control.clone(),
    };
    let backends = RepositoryBackends::new(Arc::new(backend), None);
    Repository::new_with_progress(
        &RepositoryOptions::default().no_cache(true),
        &backends,
        control.clone(),
    )
    .map_err(|_| control.error("configure repository"))
}

fn validate_name(name: &str) -> Result<(), BackupError> {
    if name.chars().count() > 80 || name.chars().any(char::is_control) {
        Err(BackupError::InvalidInput)
    } else {
        Ok(())
    }
}

fn info(snapshot: SnapshotFile) -> SnapshotInfo {
    let automatic = snapshot.tags.iter().any(|tag| tag == AUTOMATIC_TAG);
    let summary = snapshot.summary.as_ref();
    let protected = !snapshot.tags.iter().any(|tag| tag == RETENTION_TAG)
        || snapshot.tags.iter().any(|tag| tag == NAMED_TAG);
    SnapshotInfo {
        id: snapshot.id.to_hex().to_string(),
        world_id: snapshot.label,
        name: snapshot.description.unwrap_or_default(),
        created_at: snapshot.time.to_string(),
        processed_bytes: summary.map_or(0, |s| s.total_bytes_processed),
        added_bytes: summary.map_or(0, |s| s.data_added_packed),
        protected,
        automatic,
    }
}

fn identity(value: &str) -> Result<(), BackupError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(BackupError::InvalidInput);
    }
    Ok(())
}

pub(crate) fn plain_dir(path: &Path) -> Result<PathBuf, BackupError> {
    let metadata = fs::symlink_metadata(path)?;
    if linked(&metadata) || !metadata.is_dir() {
        return Err(BackupError::UnsafeSource);
    }
    Ok(dunce::canonicalize(path)?)
}

fn linked(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

type Manifest = BTreeMap<PathBuf, (bool, u64)>;
fn manifest(
    entries: impl Iterator<Item = rustic_core::RusticResult<(PathBuf, rustic_core::repofile::Node)>>,
    control: &BackupControl,
) -> Result<Manifest, BackupError> {
    let mut result = BTreeMap::new();
    for entry in entries {
        control.check()?;
        let (path, node) = entry.map_err(|_| control.error("read snapshot manifest"))?;
        if path.as_os_str().is_empty() {
            continue;
        }
        relative(&path)?;
        if !node.is_file() && !node.is_dir() {
            return Err(BackupError::UnsafeSource);
        }
        if result
            .insert(
                path,
                (
                    node.is_dir(),
                    if node.is_file() { node.meta.size } else { 0 },
                ),
            )
            .is_some()
        {
            return Err(BackupError::UnsafeSource);
        }
    }
    Ok(result)
}

pub(crate) fn relative(path: &Path) -> Result<(), BackupError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(BackupError::UnsafeSource);
    }
    for part in path.components() {
        let name = part.as_os_str().to_str().ok_or(BackupError::UnsafeSource)?;
        let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
        if name.contains([':', '\\'])
            || name.chars().any(char::is_control)
            || name.ends_with(['.', ' '])
            || matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((base.starts_with("COM") || base.starts_with("LPT"))
                && base.len() == 4
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(BackupError::UnsafeSource);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_allow_eighty_unicode_characters_and_reject_eighty_one() {
        assert!(validate_name(&"世".repeat(80)).is_ok());
        assert!(validate_name(&"世".repeat(81)).is_err());
        assert!(validate_name("hello\nworld").is_err());
        assert!(validate_name("").is_ok());
    }
}

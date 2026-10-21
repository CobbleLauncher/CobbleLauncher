// SPDX-License-Identifier: MIT
//! Capability-rooted, single-owner download staging. SQLite owns the checkpoint
//! format; the database is in memory and only a bounded serialized image is
//! published through held directory handles. No SQLite VFS receives a host path.
//!
//! Data is synced before publishing its prefix hash/offset. These are separate
//! filesystem operations, not a filesystem/SQLite/CAS transaction. Recovery
//! trusts neither stored request identity nor bytes: callers provide the former
//! anew, and the existing prefix is rehashed before it is offered for resume.
//! The final whole-file, all-digest CAS verification is still mandatory.
//!
//! Cooperating processes share a lifecycle lock and exclusively lock each job.
//! The stage must remain owned by any live network/disk worker after cancellation.
//! Locks are not a sandbox against arbitrary same-user native programs.

use crate::archive::{check_directory, check_regular, open_parent};
use crate::cas::{durability, sync_dir};
use crate::{PublicationDurability, SafeRelativePath};
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use rusqlite::{Connection, MAIN_DB, config::DbConfig, limits::Limit, params};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::TryLockError,
    io::{self, Read, Seek, SeekFrom, Write},
};

const HARD_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const IMAGE_BYTES: usize = 64 * 1024;
const APPLICATION_ID: i64 = 0x434f4244;
const VERSION: i64 = 1;
const PART: &str = "partial.part";
const SNAPSHOT: &str = "checkpoint.sqlite";
const SCHEMA: &str = "CREATE TABLE checkpoint(slot INTEGER PRIMARY KEY CHECK(slot=1),request_key BLOB NOT NULL CHECK(length(request_key)=32),expected_size INTEGER NOT NULL,max_bytes INTEGER NOT NULL,durable_offset INTEGER NOT NULL,prefix_sha256 BLOB NOT NULL CHECK(length(prefix_sha256)=32),validator BLOB) STRICT";

#[derive(Debug)]
pub enum StageError {
    Io(io::Error),
    UnsafeObject,
    InvalidIdentity,
    Limit,
    Busy,
    InvalidCheckpoint,
    UnsupportedCheckpointVersion,
    IdentityMismatch,
    RandomUnavailable,
    /// The new snapshot may already be visible. Keep the part and reopen the
    /// job after releasing ownership; never interpret this as proof of absence.
    CheckpointMayExist,
}
impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Never include a provider URL, validator or SQLite source message.
        f.write_str(match self {
            Self::Io(_) => "download staging I/O failed",
            Self::UnsafeObject => "unsafe staging filesystem object",
            Self::InvalidIdentity => "invalid download identity or budget",
            Self::Limit => "download staging limit exceeded",
            Self::Busy => "download stage has an active owner",
            Self::InvalidCheckpoint => "invalid download checkpoint",
            Self::UnsupportedCheckpointVersion => "unsupported download checkpoint version",
            Self::IdentityMismatch => "checkpoint request identity mismatch",
            Self::RandomUnavailable => "OS randomness unavailable",
            Self::CheckpointMayExist => "checkpoint may exist; durability needs recovery",
        })
    }
}
impl std::error::Error for StageError {}
impl From<io::Error> for StageError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// The trusted caller hashes a canonical request identity including every
/// expected digest, size, exact source, provenance and permission partition.
/// Never derive this from a filename, URL alone, token or transient CredentialId.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StageIdentity {
    pub request_key: [u8; 32],
    pub expected_size: u64,
    pub max_bytes: u64,
}
impl StageIdentity {
    fn validate(self) -> Result<Self, StageError> {
        if self.max_bytes > HARD_BYTES || self.expected_size > self.max_bytes {
            return Err(StageError::InvalidIdentity);
        }
        Ok(self)
    }
}
/// Opaque validator bytes are bounded storage only. The network owner must
/// validate their HTTP meaning (for example a strong ETag) before using them.
#[derive(Clone)]
pub struct DownloadCheckpoint {
    pub durable_offset: u64,
    pub prefix_sha256: [u8; 32],
    pub validator: Option<Vec<u8>>,
}
pub enum StageRecovery {
    Full,
    Resume(DownloadCheckpoint),
}

pub struct DownloadStaging {
    root: Dir,
    jobs: Dir,
}
pub struct DownloadStage {
    root: Dir,
    jobs: Dir,
    job: Dir,
    file: File,
    identity: StageIdentity,
    recovery: StageRecovery,
    prefix: Option<PrefixHash>,
    #[cfg(test)]
    prefix_bytes_read: u64,
    _lifecycle: std::fs::File,
    _owner: std::fs::File,
}
struct PrefixHash {
    hash: Sha256,
    length: u64,
}
impl DownloadStaging {
    pub fn open(registered_download_root: Dir) -> Result<Self, StageError> {
        check_directory(&registered_download_root).map_err(|_| StageError::UnsafeObject)?;
        let jobs = open_parent(
            &registered_download_root,
            &SafeRelativePath::parse("jobs").expect("constant path"),
            true,
        )
        .map_err(|_| StageError::UnsafeObject)?;
        Ok(Self {
            root: registered_download_root,
            jobs,
        })
    }

    pub fn open_job(&self, identity: StageIdentity) -> Result<DownloadStage, StageError> {
        let identity = identity.validate()?;
        let lifecycle = lease(&self.root, ".lifecycle.lock", false)?;
        let name = hex(&identity.request_key);
        let job = open_parent(
            &self.jobs,
            &SafeRelativePath::parse(&name).expect("hex path"),
            true,
        )
        .map_err(|_| StageError::UnsafeObject)?;
        let owner = lease(&job, ".job.lock", true)?;
        // Validate any snapshot before changing the existing part.
        let checkpoint = load_checkpoint(&job, identity)?;
        let file = open_file(&job, PART, true, true)?;
        let mut stage = DownloadStage {
            root: self.root.try_clone()?,
            jobs: self.jobs.try_clone()?,
            job,
            file,
            identity,
            recovery: StageRecovery::Full,
            prefix: None,
            #[cfg(test)]
            prefix_bytes_read: 0,
            _lifecycle: lifecycle,
            _owner: owner,
        };
        match checkpoint {
            Some(checkpoint)
                if stage.byte_len()? >= checkpoint.durable_offset
                    && stage.hash_prefix(checkpoint.durable_offset)?
                        == checkpoint.prefix_sha256 =>
            {
                // Unpublished bytes cannot advance recovery, even if their
                // contents happen to be valid. Discard that tail and sync.
                if stage.byte_len()? > checkpoint.durable_offset {
                    stage.file.set_len(checkpoint.durable_offset)?;
                    stage.file.sync_all()?;
                }
                stage
                    .file
                    .seek(SeekFrom::Start(checkpoint.durable_offset))?;
                if checkpoint.durable_offset > 0 {
                    stage.recovery = StageRecovery::Resume(checkpoint);
                }
            }
            _ => {
                stage.reset_for_full(None)?;
            }
        }
        Ok(stage)
    }

    /// Only exact private snapshot-temp names are removed, and only while an
    /// OS-exclusive lifecycle lease proves there is no cooperating live stage.
    /// Partial data/checkpoints and unknown objects are retained. No auto-cleanup.
    pub fn cleanup_stale_snapshots_exclusive(&mut self) -> Result<u32, StageError> {
        let _exclusive = lease(&self.root, ".lifecycle.lock", true)?;
        let mut removed = 0;
        for (seen, entry) in self.jobs.entries()?.enumerate() {
            if seen >= 100_000 {
                return Err(StageError::Limit);
            }
            let name = entry?.file_name();
            let Some(name) = name.to_str().filter(|name| is_hex(name, 64)) else {
                continue;
            };
            let job = self.jobs.open_dir_nofollow(name)?;
            check_directory(&job).map_err(|_| StageError::UnsafeObject)?;
            for (seen, entry) in job.entries()?.enumerate() {
                if seen >= 32 {
                    return Err(StageError::Limit);
                }
                let name = entry?.file_name();
                let Some(name) = name.to_str().filter(|name| snapshot_temp_name(name)) else {
                    continue;
                };
                drop(open_file(&job, name, false, false)?);
                job.remove_file(name)?;
                removed += 1;
            }
            sync_dir(&job)?;
        }
        Ok(removed)
    }
}

impl DownloadStage {
    pub fn recovery(&self) -> &StageRecovery {
        &self.recovery
    }
    pub fn byte_len(&self) -> Result<u64, StageError> {
        let metadata = self.file.metadata()?;
        regular_single_link(&metadata)?;
        Ok(metadata.len())
    }
    /// The caller has accepted a fresh full response. Truncate the same held
    /// inode; a crash before the zero checkpoint makes the old prefix fail
    /// validation on restart. Never append a 200 response to an old partial.
    pub fn reset_for_full(
        &mut self,
        validator: Option<&[u8]>,
    ) -> Result<PublicationDurability, StageError> {
        validate_validator(validator)?;
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.sync_all()?;
        self.prefix = Some(PrefixHash {
            hash: Sha256::new(),
            length: 0,
        });
        #[cfg(test)]
        fault_point("full_reset");
        self.checkpoint(validator)
    }
    /// Sync bytes, then publish the entire prefix digest and a complete SQLite
    /// image. Sequential writes update a process-local hasher; checkpoints clone
    /// its fixed-size state and do not reread old bytes. Recovery/nonappend writes
    /// rebuild once. No hasher state is serialized. Schedule on a disk worker.
    pub fn checkpoint(
        &mut self,
        validator: Option<&[u8]>,
    ) -> Result<PublicationDurability, StageError> {
        validate_validator(validator)?;
        let offset = self.byte_len()?;
        if offset > self.identity.expected_size {
            return Err(StageError::Limit);
        }
        self.file.sync_all()?;
        #[cfg(test)]
        fault_point("partial_synced");
        let checkpoint = DownloadCheckpoint {
            durable_offset: offset,
            prefix_sha256: self.hash_prefix(offset)?,
            validator: validator.map(Vec::from),
        };
        let image = serialize_checkpoint(self.identity, &checkpoint)?;
        publish_checkpoint(&self.job, &image)?;
        #[cfg(test)]
        fault_point("checkpoint_published");
        sync_dir(&self.job)
            .and_then(|_| sync_dir(&self.jobs))
            .and_then(|_| sync_dir(&self.root))
            .map_err(|_| StageError::CheckpointMayExist)?;
        self.recovery = if offset > 0 {
            StageRecovery::Resume(checkpoint)
        } else {
            StageRecovery::Full
        };
        Ok(durability())
    }
    fn hash_prefix(&mut self, length: u64) -> Result<[u8; 32], StageError> {
        if let Some(prefix) = &self.prefix
            && prefix.length == length
        {
            return Ok(prefix.hash.clone().finalize().into());
        }
        let old = self.file.stream_position()?;
        self.file.seek(SeekFrom::Start(0))?;
        let result = (|| {
            let mut hash = Sha256::new();
            let mut left = length;
            let mut buffer = [0_u8; 64 * 1024];
            while left > 0 {
                let count = self
                    .file
                    .read(&mut buffer[..left.min(64 * 1024) as usize])?;
                if count == 0 {
                    return Err(StageError::InvalidCheckpoint);
                }
                #[cfg(test)]
                {
                    self.prefix_bytes_read += count as u64;
                }
                hash.update(&buffer[..count]);
                left -= count as u64;
            }
            let digest = hash.clone().finalize().into();
            self.prefix = Some(PrefixHash { hash, length });
            Ok(digest)
        })();
        self.file.seek(SeekFrom::Start(old))?;
        result
    }
}
impl Read for DownloadStage {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Write for DownloadStage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let at = self.file.stream_position()?;
        if bytes.len() as u64 > self.identity.expected_size.saturating_sub(at) {
            return Err(io::Error::other("staging byte budget exceeded"));
        }
        let written = self.file.write(bytes)?;
        if written > 0 {
            if let Some(prefix) = &mut self.prefix
                && prefix.length == at
            {
                prefix.hash.update(&bytes[..written]);
                prefix.length += written as u64;
            } else {
                self.prefix = None;
            }
        }
        #[cfg(test)]
        fault_point("partial_written");
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl Seek for DownloadStage {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let at = match position {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::Current(delta) => self.file.stream_position()?.checked_add_signed(delta),
            SeekFrom::End(delta) => self.file.metadata()?.len().checked_add_signed(delta),
        }
        .filter(|at| *at <= self.identity.expected_size)
        .ok_or_else(|| io::Error::other("staging seek outside budget"))?;
        self.file.seek(SeekFrom::Start(at))
    }
}

fn validate_validator(value: Option<&[u8]>) -> Result<(), StageError> {
    if value.is_some_and(|value| value.len() > 1024) {
        return Err(StageError::Limit);
    }
    Ok(())
}
fn regular_single_link(metadata: &cap_std::fs::Metadata) -> Result<(), StageError> {
    check_regular(metadata).map_err(|_| StageError::UnsafeObject)?;
    if metadata.nlink() != 1 {
        return Err(StageError::UnsafeObject);
    }
    Ok(())
}
fn open_file(dir: &Dir, name: &str, writable: bool, create: bool) -> Result<File, StageError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(writable)
        .create(create)
        .follow(FollowSymlinks::No);
    let file = dir.open_with(name, &options)?;
    regular_single_link(&file.metadata()?)?;
    Ok(file)
}
fn lease(dir: &Dir, name: &str, exclusive: bool) -> Result<std::fs::File, StageError> {
    let file = open_file(dir, name, true, true)?.into_std();
    let result = if exclusive {
        file.try_lock()
    } else {
        file.try_lock_shared()
    };
    match result {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StageError::Busy),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}
fn hardened_connection() -> Result<Connection, StageError> {
    let connection = Connection::open_in_memory().map_err(|_| StageError::InvalidCheckpoint)?;
    let harden = || -> rusqlite::Result<()> {
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
        connection.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
        for (limit, value) in [
            (Limit::SQLITE_LIMIT_LENGTH, 4096),
            (Limit::SQLITE_LIMIT_SQL_LENGTH, 4096),
            (Limit::SQLITE_LIMIT_COLUMN, 16),
            (Limit::SQLITE_LIMIT_EXPR_DEPTH, 32),
            (Limit::SQLITE_LIMIT_COMPOUND_SELECT, 2),
            (Limit::SQLITE_LIMIT_VDBE_OP, 10_000),
            (Limit::SQLITE_LIMIT_ATTACHED, 0),
            (Limit::SQLITE_LIMIT_TRIGGER_DEPTH, 0),
            (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
        ] {
            connection.set_limit(limit, value)?;
        }
        connection.execute_batch(
            "PRAGMA page_size=4096; PRAGMA max_page_count=16; PRAGMA temp_store=MEMORY;",
        )?;
        Ok(())
    };
    harden().map_err(|_| StageError::InvalidCheckpoint)?;
    Ok(connection)
}
fn serialize_checkpoint(
    identity: StageIdentity,
    checkpoint: &DownloadCheckpoint,
) -> Result<Vec<u8>, StageError> {
    let mut connection = hardened_connection()?;
    let mut save = || -> rusqlite::Result<()> {
        let transaction = connection.transaction()?;
        transaction.execute_batch(SCHEMA)?;
        transaction.pragma_update(None, "user_version", VERSION)?;
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
        transaction.execute(
            "INSERT INTO checkpoint VALUES(1,?1,?2,?3,?4,?5,?6)",
            params![
                identity.request_key.as_slice(),
                identity.expected_size as i64,
                identity.max_bytes as i64,
                checkpoint.durable_offset as i64,
                checkpoint.prefix_sha256.as_slice(),
                checkpoint.validator.as_deref()
            ],
        )?;
        transaction.commit()
    };
    save().map_err(|_| StageError::InvalidCheckpoint)?;
    let image = connection
        .serialize(MAIN_DB)
        .map_err(|_| StageError::InvalidCheckpoint)?;
    if image.is_empty() || image.len() > IMAGE_BYTES {
        return Err(StageError::Limit);
    }
    Ok(image.to_vec())
}
fn load_checkpoint(
    dir: &Dir,
    identity: StageIdentity,
) -> Result<Option<DownloadCheckpoint>, StageError> {
    let mut file = match open_file(dir, SNAPSHOT, false, false) {
        Ok(file) => file,
        Err(StageError::Io(error)) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let size = file.metadata()?.len();
    if size == 0 || size > IMAGE_BYTES as u64 {
        return Err(StageError::InvalidCheckpoint);
    }
    let mut image = Vec::with_capacity(size as usize);
    (&mut file)
        .take(IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut image)?;
    if image.len() != size as usize {
        return Err(StageError::InvalidCheckpoint);
    }
    drop(file);
    validate_image_profile(&image)?;
    let mut connection = hardened_connection()?;
    // Never give rusqlite's allocating read-exact wrapper a fallible disk
    // reader. This already-filled bounded slice is exactly the declared size.
    connection
        .deserialize_read_exact(
            MAIN_DB,
            io::Cursor::new(image.as_slice()),
            image.len(),
            true,
        )
        .map_err(|_| StageError::InvalidCheckpoint)?;
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|_| StageError::InvalidCheckpoint)?;
    if version != VERSION {
        return Err(StageError::UnsupportedCheckpointVersion);
    }
    let load = || -> rusqlite::Result<DownloadCheckpoint> {
        let application: i64 =
            connection.pragma_query_value(None, "application_id", |row| row.get(0))?;
        let pages: i64 = connection.pragma_query_value(None, "page_count", |row| row.get(0))?;
        let page_size: i64 = connection.pragma_query_value(None, "page_size", |row| row.get(0))?;
        if application != APPLICATION_ID || !(1..=16).contains(&pages) || page_size != 4096 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let mut schema =
            connection.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY name")?;
        let objects = schema
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if objects
            != [(
                "table".into(),
                "checkpoint".into(),
                "checkpoint".into(),
                SCHEMA.into(),
            )]
        {
            return Err(rusqlite::Error::InvalidQuery);
        }
        let integrity: String =
            connection.query_row("PRAGMA integrity_check(1)", [], |row| row.get(0))?;
        let count: i64 =
            connection.query_row("SELECT count(*) FROM checkpoint", [], |row| row.get(0))?;
        if integrity != "ok" || count != 1 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        connection.query_row("SELECT request_key,expected_size,max_bytes,durable_offset,prefix_sha256,validator FROM checkpoint WHERE slot=1", [], |row| {
            let key: Vec<u8> = row.get(0)?;
            let size: i64 = row.get(1)?;
            let budget: i64 = row.get(2)?;
            if key != identity.request_key || size != identity.expected_size as i64 || budget != identity.max_bytes as i64 {
                return Err(rusqlite::Error::InvalidParameterName("identity".into()));
            }
            let offset: i64 = row.get(3)?;
            let prefix: Vec<u8> = row.get(4)?;
            let validator: Option<Vec<u8>> = row.get(5)?;
            if offset < 0 || offset as u64 > identity.expected_size || prefix.len() != 32 || validator.as_ref().is_some_and(|v| v.len() > 1024) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(DownloadCheckpoint { durable_offset: offset as u64, prefix_sha256: prefix.try_into().expect("length checked"), validator })
        })
    };
    match load() {
        Ok(checkpoint) => Ok(Some(checkpoint)),
        Err(rusqlite::Error::InvalidParameterName(name)) if name == "identity" => {
            Err(StageError::IdentityMismatch)
        }
        Err(_) => Err(StageError::InvalidCheckpoint),
    }
}

fn validate_image_profile(image: &[u8]) -> Result<(), StageError> {
    // Only the fixed SQLite file header is inspected. SQLite still parses all
    // pages/schema/rows. The profile prevents a tiny corrupt image claiming a
    // huge logical database before the library sees it.
    if image.is_empty()
        || image.len() > IMAGE_BYTES
        || !image.len().is_multiple_of(4096)
        || &image[..16] != b"SQLite format 3\0"
        || image[16..18] != [0x10, 0x00]
        || image[18..20] != [1, 1]
        || image[56..60] != [0, 0, 0, 1]
        || u32::from_be_bytes(image[28..32].try_into().expect("header")) as usize
            != image.len() / 4096
        || image[24..28] != image[92..96]
    {
        return Err(StageError::InvalidCheckpoint);
    }
    Ok(())
}

struct SnapshotTemp {
    dir: Dir,
    name: String,
    file: Option<File>,
    published: bool,
}
impl Drop for SnapshotTemp {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.published {
            let _ = self.dir.remove_file(&self.name);
        }
    }
}
fn publish_checkpoint(dir: &Dir, image: &[u8]) -> Result<(), StageError> {
    // Refuse unsafe existing leaves; replacement itself operates on the name
    // in the held job directory and never follows a swapped link target.
    match open_file(dir, SNAPSHOT, false, false) {
        Ok(file) => drop(file),
        Err(StageError::Io(error)) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let owned_dir = dir.try_clone()?;
    for _ in 0..16 {
        let mut random = [0; 16];
        getrandom::fill(&mut random).map_err(|_| StageError::RandomUnavailable)?;
        let name = format!("checkpoint-{}.tmp", hex(&random));
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        let file = match dir.open_with(&name, &options) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let mut temp = SnapshotTemp {
            dir: owned_dir,
            name,
            file: Some(file),
            published: false,
        };
        regular_single_link(&temp.file.as_ref().expect("created").metadata()?)?;
        temp.file.as_mut().expect("created").write_all(image)?;
        temp.file.take().expect("created").sync_all()?;
        #[cfg(test)]
        fault_point("image_synced");
        dir.rename(&temp.name, dir, SNAPSHOT)?;
        temp.published = true;
        return Ok(());
    }
    Err(StageError::RandomUnavailable)
}
fn is_hex(name: &str, length: usize) -> bool {
    name.len() == length
        && name
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}
fn snapshot_temp_name(name: &str) -> bool {
    name.strip_prefix("checkpoint-")
        .and_then(|s| s.strip_suffix(".tmp"))
        .is_some_and(|s| is_hex(s, 32))
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
fn fault_point(phase: &str) {
    if std::env::var("COBBLE_STAGE_CHILD_PHASE").ok().as_deref() == Some(phase) {
        std::fs::write(
            std::env::var_os("COBBLE_STAGE_CHILD_SIGNAL").expect("child signal"),
            b"ready",
        )
        .unwrap();
        loop {
            std::thread::park_timeout(std::time::Duration::from_secs(1));
        }
    }
}

#[cfg(test)]
#[path = "staging_tests.rs"]
mod tests;

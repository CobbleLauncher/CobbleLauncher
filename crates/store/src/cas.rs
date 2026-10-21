// SPDX-License-Identifier: MIT
//! Immutable content store. No SQLite/revision transaction or GC is implied.
//!
//! Roots are registered directory capabilities, never renderer/plugin paths.
//! All cooperating writers use a shared lifecycle lease; stale-part cleanup
//! requires a nonblocking exclusive lease on the same persistent lock file.

use std::fmt;
use std::fs::TryLockError;
use std::io::{self, Read, Write};

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use sha2::{Digest, Sha256, Sha512};

use crate::archive::{check_directory, check_regular, open_parent};
use crate::{ArchiveError, SafeRelativePath};

const HARD_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const CAS_PART_PREFIX: &str = "cas-";
const COPY_PART_PREFIX: &str = ".cobble-copy-";

#[derive(Debug)]
pub enum CasError {
    Io(io::Error),
    UnsafePath(ArchiveError),
    InvalidExpectation,
    Limit,
    SizeMismatch,
    DigestMismatch(&'static str),
    CorruptExisting,
    NotFound,
    Busy,
    RandomUnavailable,
    /// Publication may already exist and is verified. This error must never be
    /// interpreted as proof of absence or cause an unchecked overwrite/retry.
    PublicationMayExist(io::Error),
}

impl fmt::Display for CasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "content-store I/O failed: {error}"),
            Self::UnsafePath(error) => write!(f, "unsafe content-store path: {error}"),
            Self::InvalidExpectation => {
                f.write_str("invalid integrity expectation or missing upstream digest")
            }
            Self::Limit => f.write_str("artifact byte or enumeration limit exceeded"),
            Self::SizeMismatch => f.write_str("artifact size mismatch"),
            Self::DigestMismatch(algorithm) => write!(f, "artifact {algorithm} mismatch"),
            Self::CorruptExisting => {
                f.write_str("existing content-addressed artifact failed verification")
            }
            Self::NotFound => f.write_str("content-addressed artifact not found"),
            Self::Busy => f.write_str("content store has an active writer or cleanup"),
            Self::RandomUnavailable => f.write_str("OS randomness unavailable"),
            Self::PublicationMayExist(error) => write!(
                f,
                "verified publication may exist; durability/cleanup failed: {error}"
            ),
        }
    }
}
impl std::error::Error for CasError {}
impl From<io::Error> for CasError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<ArchiveError> for CasError {
    fn from(value: ArchiveError) -> Self {
        Self::UnsafePath(value)
    }
}

/// Expected digests supplied by a trusted Core caller. SHA-1 is ecosystem
/// compatibility only; it is not a modern adversarial collision guarantee.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExpectedDigests {
    pub sha256: Option<[u8; 32]>,
    pub sha1: Option<[u8; 20]>,
    pub sha512: Option<[u8; 64]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityBasis {
    /// Core has already authorized the upstream metadata/provenance. This
    /// module checks bytes against that metadata; it cannot authenticate it.
    TrustedMetadata,
    /// Explicitly confirmed local content; no upstream identity is claimed.
    UserConfirmedLocal,
}

#[derive(Clone, Copy, Debug)]
pub struct ArtifactExpectation {
    pub expected_size: Option<u64>,
    pub digests: ExpectedDigests,
    /// Installation budget, always bounded by the 8 GiB ingestion hard cap.
    pub max_bytes: u64,
    pub basis: IntegrityBasis,
}

impl ArtifactExpectation {
    fn validate(&self) -> Result<(), CasError> {
        if self.max_bytes > HARD_MAX_BYTES
            || self.expected_size.is_some_and(|size| size > self.max_bytes)
            || (self.basis == IntegrityBasis::TrustedMetadata
                && self.digests == ExpectedDigests::default())
        {
            return Err(CasError::InvalidExpectation);
        }
        Ok(())
    }
}

/// A verified content identity, not a filesystem path, bearer permission or
/// promise that external same-user software cannot mutate the backing file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedArtifact {
    sha256: [u8; 32],
    size: u64,
    expected: ExpectedDigests,
    basis: IntegrityBasis,
}

impl VerifiedArtifact {
    pub fn sha256(&self) -> [u8; 32] {
        self.sha256
    }
    pub fn sha256_hex(&self) -> String {
        hex(&self.sha256)
    }
    pub fn size_bytes(&self) -> u64 {
        self.size
    }
    pub fn expected_digests(&self) -> ExpectedDigests {
        self.expected
    }
    pub fn integrity_basis(&self) -> IntegrityBasis {
        self.basis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationDurability {
    /// File data/metadata and affected directory handles were synced. Actual
    /// power-loss behavior remains filesystem/hardware dependent.
    FileAndDirectoriesSynced,
    /// File data was synced; directory entry crash durability is unverified.
    /// This is normal on Windows, and conservative after publication recovery
    /// on any OS. No complete directory-tree durability is promised.
    FileSyncedDirectoryDurabilityUnverified,
}

#[derive(Debug)]
pub struct IngestResult {
    pub artifact: VerifiedArtifact,
    pub reused: bool,
    pub durability: PublicationDurability,
}

pub struct ContentStore {
    content_root: Dir,
    sha256_root: Dir,
    parts: Dir,
}

impl ContentStore {
    pub fn open(registered_content_root: Dir) -> Result<Self, CasError> {
        check_directory(&registered_content_root)?;
        let sha256_root = open_parent(
            &registered_content_root,
            &SafeRelativePath::parse("sha256")?,
            true,
        )?;
        let parts = open_parent(&sha256_root, &SafeRelativePath::parse(".parts")?, true)?;
        Ok(Self {
            content_root: registered_content_root,
            sha256_root,
            parts,
        })
    }

    /// Every expected digest and the optional exact size are checked before
    /// no-clobber publication. An error never authorizes replacing a CAS entry.
    pub fn ingest(
        &self,
        mut input: impl Read,
        expectation: ArtifactExpectation,
    ) -> Result<IngestResult, CasError> {
        expectation.validate()?;
        let _lease = self.lease(false)?;
        let mut temporary = Temporary::new(&self.parts, CAS_PART_PREFIX)?;
        let artifact = stream_verified(&mut input, temporary.file_mut(), expectation)?;
        temporary.sync_close()?;
        let digest = artifact.sha256_hex();
        let prefix = open_parent(
            &self.sha256_root,
            &SafeRelativePath::parse(&digest[..2])?,
            true,
        )?;
        let reused = match temporary.dir.hard_link(&temporary.name, &prefix, &digest) {
            Ok(()) => false,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                self.verify_at(&prefix, &artifact)?;
                true
            }
            Err(error) => return Err(error.into()),
        };
        // Only the already-verified inode has been linked. The writable handle
        // is closed and the transient CAS-to-part alias is removed immediately.
        temporary.remove().map_err(CasError::PublicationMayExist)?;
        sync_dir(&prefix).map_err(CasError::PublicationMayExist)?;
        sync_dir(&self.parts).map_err(CasError::PublicationMayExist)?;
        sync_dir(&self.sha256_root).map_err(CasError::PublicationMayExist)?;
        sync_dir(&self.content_root).map_err(CasError::PublicationMayExist)?;
        Ok(IngestResult {
            artifact,
            reused,
            durability: durability(),
        })
    }

    /// Reopen an expected CAS identity after restart without re-ingestion.
    /// The digest is a content identity from authorized caller metadata, not an
    /// arbitrary host path. All supplied hashes, size and budgets are checked
    /// again. Missing content is distinct from corruption and I/O failure.
    pub fn open_verified(
        &self,
        sha256: [u8; 32],
        expectation: ArtifactExpectation,
    ) -> Result<VerifiedArtifact, CasError> {
        self.open_verified_inner(sha256, expectation, None)
    }
    /// Cooperative full-file confirmation for download recovery. Check the
    /// typed stop/deadline between bounded reads; never publish on this path.
    pub fn open_verified_with_budget(
        &self,
        sha256: [u8; 32],
        expectation: ArtifactExpectation,
        budget: &crate::StageRecoveryBudget,
    ) -> Result<VerifiedArtifact, CasError> {
        self.open_verified_inner(sha256, expectation, Some(budget))
    }
    fn open_verified_inner(
        &self,
        sha256: [u8; 32],
        mut expectation: ArtifactExpectation,
        budget: Option<&crate::StageRecoveryBudget>,
    ) -> Result<VerifiedArtifact, CasError> {
        if expectation
            .digests
            .sha256
            .is_some_and(|digest| digest != sha256)
        {
            return Err(CasError::InvalidExpectation);
        }
        expectation.digests.sha256 = Some(sha256);
        expectation.validate()?;
        let _lease = self.lease(false)?;
        let digest = hex(&sha256);
        let prefix = self
            .sha256_root
            .open_dir_nofollow(&digest[..2])
            .map_err(absent)?;
        check_directory(&prefix)?;
        let input = open_regular(&prefix, &digest).map_err(|error| match error {
            CasError::Io(error) => absent(error),
            other => other,
        })?;
        let mut input = BudgetRead {
            file: input,
            budget,
        };
        stream_verified(&mut input, &mut io::sink(), expectation).map_err(existing_error)
    }

    /// Copy into a dedicated caller-owned instance staging root. Existing target
    /// leaves are refused. The source is hashed while copying; stale handles or
    /// corrupt backing bytes cannot produce a successful new instance file.
    ///
    /// The target is an independent inode, never a CAS hardlink. Installation
    /// into a running instance and transactional replacement belong to T009/10.
    pub fn copy_to(
        &self,
        artifact: &VerifiedArtifact,
        registered_staging_root: &Dir,
        target: &SafeRelativePath,
    ) -> Result<PublicationDurability, CasError> {
        let _lease = self.lease(false)?;
        check_directory(registered_staging_root)?;
        let digest = artifact.sha256_hex();
        let prefix = self.sha256_root.open_dir_nofollow(&digest[..2])?;
        check_directory(&prefix)?;
        let mut source = open_regular(&prefix, &digest)?;
        copy_verified_input(&mut source, artifact, registered_staging_root, target)
    }

    /// Restart recovery only: acquire a real exclusive CAS lifecycle lease.
    /// Returns Busy while any cooperating process/handle is ingesting/copying.
    /// No cleanup runs implicitly on open. Only exact scoped CAS part names are
    /// removed; unexpected names are preserved, links/nonregular parts refused.
    /// This is not the instance/process ownership protocol required by T010.
    pub fn cleanup_stale_parts_exclusive(&mut self) -> Result<u32, CasError> {
        let _lease = self.lease(true)?;
        let mut removed = 0;
        for (seen, entry) in self.parts.entries()?.enumerate() {
            if seen >= 100_000 {
                return Err(CasError::Limit);
            }
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !is_cas_part_name(name) {
                continue;
            }
            drop(open_regular(&self.parts, name)?);
            self.parts.remove_file(name)?;
            removed += 1;
        }
        sync_dir(&self.parts)?;
        Ok(removed)
    }

    fn verify_at(&self, prefix: &Dir, expected: &VerifiedArtifact) -> Result<(), CasError> {
        let mut input = open_regular(prefix, &expected.sha256_hex())?;
        let request = ArtifactExpectation {
            expected_size: Some(expected.size),
            digests: ExpectedDigests {
                sha256: Some(expected.sha256),
                ..expected.expected
            },
            max_bytes: expected.size,
            basis: expected.basis,
        };
        stream_verified(&mut input, &mut io::sink(), request).map_err(existing_error)?;
        Ok(())
    }

    fn lease(&self, exclusive: bool) -> Result<std::fs::File, CasError> {
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .follow(FollowSymlinks::No);
        let lock = self.sha256_root.open_with(".ingest.lock", &options)?;
        check_regular(&lock.metadata()?)?;
        // A fresh open handle is required: cloned lock handles have platform-
        // specific recursive/unlock behavior and are never used here.
        let lock = lock.into_std();
        let result = if exclusive {
            lock.try_lock()
        } else {
            lock.try_lock_shared()
        };
        match result {
            Ok(()) => Ok(lock),
            Err(TryLockError::WouldBlock) => Err(CasError::Busy),
            Err(TryLockError::Error(error)) => Err(error.into()),
        }
    }
}

struct BudgetRead<'a> {
    file: File,
    budget: Option<&'a crate::StageRecoveryBudget>,
}
impl Read for BudgetRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if let Some(budget) = self.budget {
            budget
                .check()
                .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "verification stopped"))?;
        }
        self.file.read(out)
    }
}

fn copy_verified_input(
    source: &mut impl Read,
    artifact: &VerifiedArtifact,
    registered_staging_root: &Dir,
    target: &SafeRelativePath,
) -> Result<PublicationDurability, CasError> {
    let parent = open_parent(registered_staging_root, target, false)?;
    let mut temporary = Temporary::new(&parent, COPY_PART_PREFIX)?;
    let expectation = ArtifactExpectation {
        expected_size: Some(artifact.size),
        digests: ExpectedDigests {
            sha256: Some(artifact.sha256),
            ..artifact.expected
        },
        max_bytes: artifact.size,
        basis: artifact.basis,
    };
    stream_verified(source, temporary.file_mut(), expectation).map_err(existing_error)?;
    temporary.sync_close()?;
    let leaf = target
        .as_str()
        .rsplit('/')
        .next()
        .expect("validated nonempty path");
    temporary.dir.hard_link(&temporary.name, &parent, leaf)?;
    temporary.remove().map_err(CasError::PublicationMayExist)?;
    sync_dir(&parent).map_err(CasError::PublicationMayExist)?;
    sync_parent_chain(registered_staging_root, target).map_err(CasError::PublicationMayExist)?;
    Ok(durability())
}

fn existing_error(error: CasError) -> CasError {
    match error {
        CasError::DigestMismatch(_) | CasError::SizeMismatch | CasError::Limit => {
            CasError::CorruptExisting
        }
        other => other,
    }
}

fn absent(error: io::Error) -> CasError {
    if error.kind() == io::ErrorKind::NotFound {
        CasError::NotFound
    } else {
        CasError::Io(error)
    }
}

fn sync_parent_chain(root: &Dir, target: &SafeRelativePath) -> io::Result<()> {
    let mut current = root.try_clone()?;
    sync_dir(&current)?;
    let parts: Vec<_> = target.as_str().split('/').collect();
    for component in &parts[..parts.len() - 1] {
        current = current.open_dir_nofollow(component)?;
        sync_dir(&current)?;
    }
    Ok(())
}

use cap_fs_ext::DirExt;

fn open_regular(dir: &Dir, name: &str) -> Result<File, CasError> {
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = dir.open_with(name, &options)?;
    check_regular(&file.metadata()?)?;
    Ok(file)
}

fn stream_verified(
    input: &mut impl Read,
    output: &mut impl Write,
    expectation: ArtifactExpectation,
) -> Result<VerifiedArtifact, CasError> {
    expectation.validate()?;
    let mut sha256 = Sha256::new();
    let mut sha1 = expectation.digests.sha1.map(|_| sha1::Sha1::new());
    let mut sha512 = expectation.digests.sha512.map(|_| Sha512::new());
    let mut size = 0_u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let capacity = expectation
            .max_bytes
            .saturating_sub(size)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let count = match input.read(&mut buffer[..capacity]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        size = size.checked_add(count as u64).ok_or(CasError::Limit)?;
        if size > expectation.max_bytes {
            return Err(CasError::Limit);
        }
        let bytes = &buffer[..count];
        sha256.update(bytes);
        if let Some(hash) = sha1.as_mut() {
            hash.update(bytes);
        }
        if let Some(hash) = sha512.as_mut() {
            hash.update(bytes);
        }
        output.write_all(bytes)?;
    }
    if expectation
        .expected_size
        .is_some_and(|expected| size != expected)
    {
        return Err(CasError::SizeMismatch);
    }
    let digest: [u8; 32] = sha256.finalize().into();
    if expectation
        .digests
        .sha256
        .is_some_and(|expected| expected != digest)
    {
        return Err(CasError::DigestMismatch("SHA-256"));
    }
    if let Some(hash) = sha1
        && expectation.digests.sha1 != Some(hash.finalize().into())
    {
        return Err(CasError::DigestMismatch("SHA-1"));
    }
    if let Some(hash) = sha512
        && expectation.digests.sha512 != Some(hash.finalize().into())
    {
        return Err(CasError::DigestMismatch("SHA-512"));
    }
    Ok(VerifiedArtifact {
        sha256: digest,
        size,
        expected: expectation.digests,
        basis: expectation.basis,
    })
}

struct Temporary {
    dir: Dir,
    name: String,
    file: Option<File>,
    armed: bool,
}

impl Temporary {
    fn new(dir: &Dir, prefix: &str) -> Result<Self, CasError> {
        let owned_dir = dir.try_clone()?;
        for _ in 0..16 {
            let mut random = [0; 16];
            getrandom::fill(&mut random).map_err(|_| CasError::RandomUnavailable)?;
            let name = format!("{prefix}{}.part", hex(&random));
            let mut options = OpenOptions::new();
            options
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No);
            match dir.open_with(&name, &options) {
                Ok(file) => {
                    let temporary = Self {
                        dir: owned_dir,
                        name,
                        file: Some(file),
                        armed: true,
                    };
                    check_regular(&temporary.file.as_ref().expect("just created").metadata()?)?;
                    return Ok(temporary);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(CasError::RandomUnavailable)
    }
    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("temporary is still open")
    }
    fn sync_close(&mut self) -> Result<(), CasError> {
        self.file
            .take()
            .expect("temporary is still open")
            .sync_all()?;
        Ok(())
    }
    fn remove(mut self) -> io::Result<()> {
        drop(self.file.take());
        self.dir.remove_file(&self.name)?;
        self.armed = false;
        Ok(())
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        drop(self.file.take());
        if self.armed {
            let _ = self.dir.remove_file(&self.name);
        }
    }
}

fn is_cas_part_name(name: &str) -> bool {
    name.strip_prefix(CAS_PART_PREFIX)
        .and_then(|value| value.strip_suffix(".part"))
        .is_some_and(|value| {
            value.len() == 32
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[usize::from(byte >> 4)] as char);
        result.push(DIGITS[usize::from(byte & 15)] as char);
    }
    result
}

pub(crate) fn sync_dir(dir: &Dir) -> io::Result<()> {
    #[cfg(unix)]
    dir.try_clone()?.into_std_file().sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

pub(crate) fn durability() -> PublicationDurability {
    #[cfg(unix)]
    {
        PublicationDurability::FileAndDirectoriesSynced
    }
    #[cfg(not(unix))]
    {
        PublicationDurability::FileSyncedDirectoryDurabilityUnverified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingInput(bool);
    impl Read for FailingInput {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.0 {
                return Err(io::Error::other("synthetic source read failure"));
            }
            self.0 = true;
            let count = 1024.min(buffer.len());
            buffer[..count].fill(b'x');
            Ok(count)
        }
    }

    #[test]
    fn store003_interrupted_copy_removes_partial_target_temp() {
        let path = std::env::temp_dir().join(format!(
            "cobble-copy-interrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let artifact = VerifiedArtifact {
            sha256: Sha256::digest([b'x'; 2048]).into(),
            size: 2048,
            expected: ExpectedDigests::default(),
            basis: IntegrityBasis::UserConfirmedLocal,
        };
        let target = SafeRelativePath::parse("mods/a.jar").unwrap();
        let result = copy_verified_input(&mut FailingInput(false), &artifact, &root, &target);
        assert!(matches!(result, Err(CasError::Io(_))));
        assert_eq!(std::fs::read_dir(path.join("mods")).unwrap().count(), 0);
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }

    struct FailingOutput;
    impl Write for FailingOutput {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("synthetic disk full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn store002_write_errors_cannot_be_verified_as_success() {
        let request = ArtifactExpectation {
            expected_size: Some(3),
            digests: ExpectedDigests::default(),
            max_bytes: 3,
            basis: IntegrityBasis::UserConfirmedLocal,
        };
        assert!(matches!(
            stream_verified(&mut &b"abc"[..], &mut FailingOutput, request),
            Err(CasError::Io(_))
        ));
    }
}

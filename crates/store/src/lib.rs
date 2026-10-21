// SPDX-License-Identifier: MIT
//! SQLite, immutable artifacts, journal recovery and instance locks.
//!
//! Version-gated SQLite initialization. Filesystem install transactions are not
//! implemented here and must never be confused with a SQLite transaction.

mod database;
pub use database::{StateStore, StoreError};

mod archive;
mod safe_path;
pub use archive::{ArchiveError, ArchiveLimits, ArchiveSummary, extract_zip};
pub use safe_path::SafeRelativePath;

mod cas;
pub use cas::{
    ArtifactExpectation, CasError, ContentStore, ExpectedDigests, IngestResult, IntegrityBasis,
    PublicationDurability, VerifiedArtifact,
};

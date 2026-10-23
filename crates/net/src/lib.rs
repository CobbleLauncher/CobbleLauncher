// SPDX-License-Identifier: MIT
//! HTTPS GET broker for authorized Core adapters. Public destinations, pinned
//! sockets, platform TLS verification, bounded HTTP/1 responses. Identity
//! encoding only; the normal broker has no proxy, automatic redirect, cookie
//! jar or retry behavior. Public loader adapters have an explicit proxy fallback.
//! This crate exposes no renderer IPC and grants no provider authorization.

mod adaptive;
mod artifact;
mod broker;
mod credentials;
mod download;
mod manager;
mod metadata_cache;
mod policy;
mod public_proxy;
mod tuning;
pub use artifact::{
    AcceptedArtifactHeaders, ArtifactFile, ArtifactTransferRequest, StrongEtag,
    TransferCancellation, TransferError, TransferMode, TransferOutcome, TransferReceipt,
};
pub use broker::{
    Broker, BrokerConfig, BufferedResponse, NetError, ResponseMetadata, TimeoutPhase,
};
pub use credentials::{CredentialId, CredentialPartition, CredentialScope, ProviderCredentials};
pub use download::{
    ArtifactDownloader, DownloadAuthority, DownloadError, DownloadRequest, DownloadResult,
};
pub use manager::{ActiveDownloadProgress, DownloadManager, DownloadMetrics, ManagedDownloadError};
pub use metadata_cache::{MetadataCacheConfig, MetadataCacheStats, PublicMetadataCache};
pub use public_proxy::fetch_loader_via_system_proxy;
pub use tuning::{DownloadTuning, download_tuning, save_download_tuning};
#[cfg(test)]
mod tests;

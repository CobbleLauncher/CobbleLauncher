// SPDX-License-Identifier: MIT
//! HTTPS GET broker for authorized Core adapters. Public destinations, pinned
//! sockets, platform TLS verification, bounded HTTP/1 responses. Identity
//! encoding only; no proxy, automatic redirect, cookie jar or retry behavior.
//! This crate exposes no renderer IPC and grants no provider authorization.

mod artifact;
mod broker;
mod credentials;
mod download;
mod manager;
mod policy;
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
pub use manager::{DownloadManager, ManagedDownloadError};
#[cfg(test)]
mod tests;

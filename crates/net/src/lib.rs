// SPDX-License-Identifier: MIT
//! HTTPS GET broker for authorized Core adapters. Public destinations, pinned
//! sockets, platform TLS verification, bounded HTTP/1 responses. Identity
//! encoding only; no proxy, automatic redirect, cookie jar or retry behavior.
//! This crate exposes no renderer IPC and grants no provider authorization.

mod broker;
mod credentials;
mod policy;
pub use broker::{
    Broker, BrokerConfig, BufferedResponse, NetError, ResponseMetadata, TimeoutPhase,
};
pub use credentials::{CredentialId, CredentialScope, ProviderCredentials};
#[cfg(test)]
mod tests;

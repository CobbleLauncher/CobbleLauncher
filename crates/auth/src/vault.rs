// SPDX-License-Identifier: MIT
//! Refresh tokens only. No mock or plaintext fallback in production.
use crate::AuthError;

pub(super) trait Vault {
    fn get(&self, id: &str) -> Result<Option<String>, AuthError>;
    fn set(&self, id: &str, token: &str) -> Result<(), AuthError>;
    fn delete(&self, id: &str) -> Result<(), AuthError>;
}
pub(super) struct SystemVault;

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn entry(id: &str) -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new("CobbleLauncher.Microsoft", id).map_err(|_| AuthError::VaultUnavailable)
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
impl Vault for SystemVault {
    fn get(&self, id: &str) -> Result<Option<String>, AuthError> {
        match entry(id)?.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AuthError::VaultUnavailable),
        }
    }
    fn set(&self, id: &str, token: &str) -> Result<(), AuthError> {
        entry(id)?
            .set_password(token)
            .map_err(|_| AuthError::VaultUnavailable)
    }
    fn delete(&self, id: &str) -> Result<(), AuthError> {
        match entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AuthError::VaultUnavailable),
        }
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
impl Vault for SystemVault {
    fn get(&self, _: &str) -> Result<Option<String>, AuthError> {
        Err(AuthError::VaultUnavailable)
    }
    fn set(&self, _: &str, _: &str) -> Result<(), AuthError> {
        Err(AuthError::VaultUnavailable)
    }
    fn delete(&self, _: &str) -> Result<(), AuthError> {
        Err(AuthError::VaultUnavailable)
    }
}

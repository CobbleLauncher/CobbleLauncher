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
    #[cfg(target_os = "linux")]
    require_unlocked_collection()?;
    keyring::Entry::new("CobbleLauncher.Microsoft", id).map_err(|_| AuthError::VaultUnavailable)
}

#[cfg(target_os = "linux")]
fn require_unlocked_collection() -> Result<(), AuthError> {
    use dbus_secret_service::{EncryptionType, SecretService};

    // keyring otherwise waits indefinitely for an unlock prompt, including on
    // headless desktops. Let the user unlock their desktop keyring and retry.
    let service = SecretService::connect_with_max_prompt_timeout(EncryptionType::Dh, 0)
        .map_err(|_| AuthError::VaultUnavailable)?;
    let collection = service
        .get_default_collection()
        .map_err(|_| AuthError::VaultUnavailable)?;
    if collection
        .is_locked()
        .map_err(|_| AuthError::VaultUnavailable)?
    {
        return Err(AuthError::VaultUnavailable);
    }
    Ok(())
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

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::{SystemVault, Vault};
    use crate::AuthError;

    fn disposable_id() -> String {
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).expect("random ID");
        let suffix: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("t090-disposable-{suffix}")
    }

    #[test]
    #[ignore = "Run only with an isolated HOME, XDG runtime and private Secret Service"]
    fn linux_system_vault_roundtrip() {
        let id = disposable_id();
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = SystemVault.delete(&self.0);
            }
        }
        let cleanup = Cleanup(id);
        let secret = "synthetic-t090-refresh-token";
        assert!(
            matches!(SystemVault.get(&cleanup.0), Ok(None)),
            "credential already exists"
        );
        SystemVault.set(&cleanup.0, secret).expect("write");
        assert!(
            SystemVault.get(&cleanup.0).expect("read").as_deref() == Some(secret),
            "credential mismatch"
        );
        SystemVault.delete(&cleanup.0).expect("delete");
        assert!(
            matches!(SystemVault.get(&cleanup.0), Ok(None)),
            "credential remains"
        );
    }

    #[test]
    #[ignore = "Run only with an isolated HOME, private Secret Service and busctl"]
    fn linux_locked_service_returns_unavailable() {
        let id = disposable_id();
        SystemVault
            .set(&id, "synthetic-t090-refresh-token")
            .expect("write before lock");
        let status = std::process::Command::new("busctl")
            .args([
                "--user",
                "call",
                "org.freedesktop.secrets",
                "/org/freedesktop/secrets",
                "org.freedesktop.Secret.Service",
                "Lock",
                "ao",
                "1",
                "/org/freedesktop/secrets/aliases/default",
            ])
            .status()
            .expect("busctl");
        assert!(status.success(), "lock request failed");
        assert!(matches!(
            SystemVault.get(&id),
            Err(AuthError::VaultUnavailable)
        ));
        assert!(matches!(
            SystemVault.set(&id, "synthetic-t090-refresh-token"),
            Err(AuthError::VaultUnavailable)
        ));
        assert!(matches!(
            SystemVault.delete(&id),
            Err(AuthError::VaultUnavailable)
        ));
    }

    #[test]
    #[ignore = "Run only with an isolated HOME, XDG runtime and no Secret Service"]
    fn linux_missing_service_never_persists() {
        let id = disposable_id();
        assert!(matches!(
            SystemVault.get(&id),
            Err(AuthError::VaultUnavailable)
        ));
        assert!(matches!(
            SystemVault.set(&id, "synthetic-t090-refresh-token"),
            Err(AuthError::VaultUnavailable)
        ));
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

// SPDX-License-Identifier: MIT
use crate::{Result, fail};
use aws_lc_rs::{rand::SystemRandom, signature::Ed25519KeyPair};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use tough::key_source::{KeySource, LocalKeySource};
pub(crate) fn source(path: &Path) -> Box<dyn KeySource> {
    Box::new(LocalKeySource { path: path.into() })
}
pub async fn public_key_id(path: &Path) -> Result<String> {
    let key = source(path)
        .as_sign()
        .await
        .map_err(|_| fail("Cannot load signing key"))?
        .tuf_key();
    let value = serde_json::to_value(key.key_id()?)?;
    Ok(value
        .as_str()
        .ok_or_else(|| fail("Invalid public key ID"))?
        .into())
}
pub fn create_key(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(fail(
            "Signing key requires an absolute outside-workspace path",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| fail("Invalid key path"))?
        .canonicalize()?;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    if parent.starts_with(workspace) {
        return Err(fail("Signing keys must stay outside the workspace"));
    }
    if fs::symlink_metadata(&parent)?.file_type().is_symlink() {
        return Err(fail("Unsafe key directory"));
    }
    let document = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
        .map_err(|_| fail("Could not generate Ed25519 key"))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(document.as_ref())?;
    file.sync_all()?;
    Ok(())
}

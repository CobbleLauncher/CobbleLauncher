// SPDX-License-Identifier: MIT
//! Explicit roots for fresh installs; local approval receipts keep installed packages usable offline.
use crate::{PluginError, PluginManifest, digest, read_regular};
use cobble_plugin_trust::{BoundFile, VerifiedPackage};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
const LIMIT: usize = 1024 * 1024;
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase", deny_unknown_fields)]
pub enum SignatureStatus {
    Unsigned,
    Verified {
        #[serde(rename = "rootId")]
        root_id: String,
        publisher: String,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    plugin_id: String,
    manifest_sha256: String,
    root_sha256: Option<String>,
    metadata_version: u64,
    bound_files: Vec<BoundFile>,
}
fn failure(code: &str) -> PluginError {
    PluginError::new(code, "Plugin signature or approval is invalid")
}
fn regular_directory(path: &Path) -> Result<(), PluginError> {
    let meta = fs::symlink_metadata(path).map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(failure("PLUGIN_SIGNATURE_INVALID"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(failure("PLUGIN_SIGNATURE_INVALID"));
        }
    }
    Ok(())
}
fn proof_present(package: &Path) -> Result<bool, PluginError> {
    let path = package.join(cobble_plugin_trust::PROOF_DIR);
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            regular_directory(&path)?;
            Ok(true)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(failure("PLUGIN_SIGNATURE_INVALID")),
    }
}
fn trusted_root(roots: Option<&Path>, id: &str) -> Result<Vec<u8>, PluginError> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(failure("PLUGIN_SIGNATURE_UNTRUSTED"));
    }
    let roots = roots.ok_or_else(|| failure("PLUGIN_SIGNATURE_UNTRUSTED"))?;
    regular_directory(roots).map_err(|_| failure("PLUGIN_SIGNATURE_UNTRUSTED"))?;
    let bytes = read_regular(roots, &format!("{id}.json"), LIMIT)
        .map_err(|_| failure("PLUGIN_SIGNATURE_UNTRUSTED"))?;
    if digest(&bytes) != id {
        return Err(failure("PLUGIN_SIGNATURE_UNTRUSTED"));
    }
    Ok(bytes)
}
pub(crate) fn verify_source(
    package: &Path,
    roots: Option<&Path>,
) -> Result<Option<VerifiedPackage>, PluginError> {
    if !proof_present(package)? {
        return Ok(None);
    }
    let metadata = package
        .join(cobble_plugin_trust::PROOF_DIR)
        .join("metadata");
    regular_directory(&metadata)?;
    let bundled = read_regular(&metadata, "1.root.json", LIMIT)
        .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
    let root = trusted_root(roots, &digest(&bundled))?;
    cobble_plugin_trust::verify_package_blocking(package, &root)
        .map(Some)
        .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))
}
pub fn package_signature_status(
    package: &Path,
    roots: Option<&Path>,
) -> Result<SignatureStatus, PluginError> {
    let verified = verify_source(package, roots)?;
    let manifest: PluginManifest =
        serde_json::from_slice(&read_regular(package, "plugin.json", 16384)?)
            .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
    Ok(match verified {
        None => SignatureStatus::Unsigned,
        Some(verified) => SignatureStatus::Verified {
            root_id: verified.root_sha256,
            publisher: manifest.publisher,
        },
    })
}
fn receipt_directory(registry: &Path, id: &str) -> PathBuf {
    registry.join(".trusted-receipts").join(id)
}
fn receipt_bytes(receipt: &Receipt) -> Result<Vec<u8>, PluginError> {
    let bytes =
        serde_json::to_vec(receipt).map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    if bytes.len() > LIMIT {
        return Err(failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"));
    }
    Ok(bytes)
}
fn receipts(registry: &Path, id: &str) -> Result<Vec<Receipt>, PluginError> {
    let directory = receipt_directory(registry, id);
    regular_directory(&registry.join(".trusted-receipts"))?;
    regular_directory(&directory)?;
    let mut result = vec![];
    for (index, entry) in fs::read_dir(&directory)
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"))?
        .take(258)
        .enumerate()
    {
        if index >= 257 {
            return Err(failure("PLUGIN_SIGNATURE_HISTORY_LIMIT"));
        }
        let entry = entry.map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"))?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"))?;
        if name.starts_with(".tmp") {
            continue;
        }
        if name == "required" {
            read_regular(&directory, name, 8)?;
            continue;
        }
        let bytes = read_regular(&directory, name, LIMIT)?;
        if name != format!("{}.json", digest(&bytes)) {
            return Err(failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"));
        }
        let receipt: Receipt = serde_json::from_slice(&bytes)
            .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"))?;
        if receipt.schema_version != 1 || receipt.plugin_id != id {
            return Err(failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"));
        }
        result.push(receipt);
    }
    Ok(result)
}
fn installed_receipt(
    registry: &Path,
    package: &Path,
    manifest: &PluginManifest,
) -> Result<Option<Receipt>, PluginError> {
    let directory = receipt_directory(registry, &manifest.id);
    if !directory.exists() {
        return Ok(None);
    }
    let hash = digest(&read_regular(package, "plugin.json", 16384)?);
    let signed = proof_present(package)?;
    if package == registry.join(&manifest.id) && directory.join("required").exists() && !signed {
        return Err(failure("PLUGIN_SIGNATURE_INVALID"));
    }
    for receipt in receipts(registry, &manifest.id)? {
        if receipt.manifest_sha256 != hash {
            continue;
        }
        match &receipt.root_sha256 {
            None if !signed => return Ok(Some(receipt)),
            Some(_)
                if signed
                    && cobble_plugin_trust::verify_bound_files(package, &receipt.bound_files)
                        .is_ok() =>
            {
                return Ok(Some(receipt));
            }
            _ => (),
        }
    }
    Err(failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED"))
}
pub(crate) fn verify_installed(
    registry: &Path,
    package: &Path,
    manifest: &PluginManifest,
    roots: Option<&Path>,
) -> Result<(), PluginError> {
    match installed_receipt(registry, package, manifest)? {
        None if proof_present(package)? => Err(failure("PLUGIN_SIGNATURE_APPROVAL_REQUIRED")),
        None => Ok(()),
        Some(receipt) => {
            if let Some(root) = receipt.root_sha256 {
                trusted_root(roots, &root)?;
                cobble_plugin_trust::verify_bound_files(package, &receipt.bound_files)
                    .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
            }
            Ok(())
        }
    }
}
pub(crate) fn installed_signature_status(
    registry: &Path,
    package: &Path,
    manifest: &PluginManifest,
    roots: Option<&Path>,
) -> Result<SignatureStatus, PluginError> {
    verify_installed(registry, package, manifest, roots)?;
    Ok(
        match installed_receipt(registry, package, manifest)?
            .and_then(|receipt| receipt.root_sha256)
        {
            Some(root_id) => SignatureStatus::Verified {
                root_id,
                publisher: manifest.publisher.clone(),
            },
            None => SignatureStatus::Unsigned,
        },
    )
}
fn persist(registry: &Path, receipt: &Receipt) -> Result<(), PluginError> {
    let directory = receipt_directory(registry, &receipt.plugin_id);
    let base = registry.join(".trusted-receipts");
    if !base.exists() {
        fs::create_dir(&base).map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    }
    regular_directory(&base)?;
    if !directory.exists() {
        fs::create_dir(&directory).map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    }
    regular_directory(&registry.join(".trusted-receipts"))?;
    regular_directory(&directory)?;
    let bytes = receipt_bytes(receipt)?;
    let path = directory.join(format!("{}.json", digest(&bytes)));
    let mut file = tempfile::NamedTempFile::new_in(&directory)
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    file.write_all(&bytes)
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    file.persist(path)
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    Ok(())
}
fn check_revision(prior: &Receipt, verified: &VerifiedPackage) -> Result<(), PluginError> {
    if prior.plugin_id == verified.plugin_id
        && prior.root_sha256.as_deref() == Some(&verified.root_sha256)
        && (prior.metadata_version > verified.metadata_version
            || (prior.metadata_version == verified.metadata_version
                && (prior.manifest_sha256 != verified.manifest_sha256
                    || prior.bound_files != verified.bound_files)))
    {
        return Err(failure("PLUGIN_SIGNATURE_ROLLBACK"));
    }
    Ok(())
}
/// Receipt filenames bind the whole approval, preserving old proof after a re-sign or failed update.
pub(crate) fn approve(
    registry: &Path,
    source: &Path,
    staging: &Path,
    manifest: &PluginManifest,
    verified: Option<VerifiedPackage>,
    previous: Option<&Path>,
) -> Result<bool, PluginError> {
    let signed = verified.is_some();
    if let Some(previous) = previous {
        let prior: PluginManifest =
            serde_json::from_slice(&read_regular(previous, "plugin.json", 16384)?)
                .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
        let existing = installed_receipt(registry, previous, &prior)?;
        if !signed
            && existing
                .as_ref()
                .is_some_and(|receipt| receipt.root_sha256.is_some())
        {
            return Err(failure("PLUGIN_SIGNATURE_DOWNGRADE"));
        }
        if signed && existing.is_none() {
            persist(
                registry,
                &Receipt {
                    schema_version: 1,
                    plugin_id: prior.id,
                    manifest_sha256: digest(&read_regular(previous, "plugin.json", 16384)?),
                    root_sha256: None,
                    metadata_version: 0,
                    bound_files: vec![],
                },
            )?
        }
    }
    if let Some(verified) = verified {
        if receipt_directory(registry, &manifest.id).exists() {
            for prior in receipts(registry, &manifest.id)? {
                check_revision(&prior, &verified)?
            }
        }
        if verified.plugin_id != manifest.id
            || verified.manifest_sha256 != digest(&read_regular(source, "plugin.json", 16384)?)
        {
            return Err(failure("PLUGIN_SIGNATURE_INVALID"));
        }
        cobble_plugin_trust::copy_package_proof(source, staging)
            .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
        cobble_plugin_trust::verify_bound_files(staging, &verified.bound_files)
            .map_err(|_| failure("PLUGIN_SIGNATURE_INVALID"))?;
        persist(
            registry,
            &Receipt {
                schema_version: 1,
                plugin_id: manifest.id.clone(),
                manifest_sha256: verified.manifest_sha256,
                root_sha256: Some(verified.root_sha256),
                metadata_version: verified.metadata_version,
                bound_files: verified.bound_files,
            },
        )?;
    } else if receipt_directory(registry, &manifest.id).exists() {
        persist(
            registry,
            &Receipt {
                schema_version: 1,
                plugin_id: manifest.id.clone(),
                manifest_sha256: digest(&read_regular(staging, "plugin.json", 16384)?),
                root_sha256: None,
                metadata_version: 0,
                bound_files: vec![],
            },
        )?
    }
    Ok(signed)
}
pub(crate) fn mark_verified(registry: &Path, id: &str) -> Result<(), PluginError> {
    let directory = receipt_directory(registry, id);
    regular_directory(&directory)?;
    let path = directory.join("required");
    if path.exists() {
        read_regular(&directory, "required", 8)?;
        return Ok(());
    }
    let file = tempfile::NamedTempFile::new_in(&directory)
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    file.as_file()
        .sync_all()
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    file.persist(path)
        .map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?;
    Ok(())
}
pub(crate) fn remove_receipts(registry: &Path, id: &str) -> Result<(), PluginError> {
    let path = receipt_directory(registry, id);
    if path.exists() {
        regular_directory(&registry.join(".trusted-receipts"))?;
        regular_directory(&path)?;
        fs::remove_dir_all(path).map_err(|_| failure("PLUGIN_SIGNATURE_APPROVAL_FAILED"))?
    }
    Ok(())
}

#[cfg(test)]
#[path = "package_trust_tests.rs"]
mod tests;

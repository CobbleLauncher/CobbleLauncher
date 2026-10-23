// SPDX-License-Identifier: MIT
use crate::{Result, fail};
use cobble_plugin_api::{
    FrontendAssetIndex, MAX_COMPONENT_BYTES, MAX_FRONTEND_INDEX_BYTES, PluginManifest,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
pub const PROOF_DIR: &str = ".cobble-signatures";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundFile {
    pub path: String,
    pub length: u64,
    pub sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Descriptor {
    pub schema_version: u32,
    pub plugin_id: String,
    pub version: String,
    pub files: Vec<BoundFile>,
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn regular(root: &Path, path: &str, limit: usize) -> Result<Vec<u8>> {
    if path.is_empty()
        || path.len() > 512
        || path.split('/').any(|p| {
            p.is_empty() || p == "." || p == ".." || p.contains(['\\', ':', '%', '?', '#'])
        })
    {
        return Err(fail("Unsafe signed package path"));
    }
    let mut current = root.to_path_buf();
    let meta = fs::symlink_metadata(&current)?;
    if !meta.is_dir() || linked(&meta) {
        return Err(fail("Unsafe package directory"));
    }
    let count = path.split('/').count();
    for (i, part) in path.split('/').enumerate() {
        current.push(part);
        let meta = fs::symlink_metadata(&current)?;
        if linked(&meta) || (i + 1 < count && !meta.is_dir()) || (i + 1 == count && !meta.is_file())
        {
            return Err(fail("Unsafe signed package entry"));
        }
    }
    let file = fs::File::open(current)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(fail("Signed package file exceeds limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(fail("Signed package file exceeds limit"));
    }
    Ok(bytes)
}
fn linked(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    meta.file_type().is_symlink()
}
pub(crate) fn bound(root: &Path, path: &str, limit: usize) -> Result<BoundFile> {
    let bytes = regular(root, path, limit)?;
    Ok(BoundFile {
        path: path.into(),
        length: bytes.len() as u64,
        sha256: hash(&bytes),
    })
}
pub(crate) fn inventory(root: &Path) -> Result<(PluginManifest, Vec<BoundFile>)> {
    let bytes = regular(root, "plugin.json", 16384)?;
    let manifest: PluginManifest = serde_json::from_slice(&bytes)?;
    manifest.validate()?;
    let mut files = vec![bound(root, "plugin.json", 16384)?];
    let component = regular(root, "component.wasm", MAX_COMPONENT_BYTES)?;
    if !component.starts_with(b"\0asm\x0d\0\x01\0")
        || hash(&component) != manifest.component_sha256.to_ascii_lowercase()
    {
        return Err(fail("Component integrity mismatch"));
    }
    files.push(bound(root, "component.wasm", MAX_COMPONENT_BYTES)?);
    if let Some(icon) = &manifest.icon {
        let file = bound(root, &icon.file, 256 * 1024)?;
        if file.sha256 != icon.sha256.to_ascii_lowercase() {
            return Err(fail("Icon integrity mismatch"));
        }
        files.push(file);
    }
    if let Some(frontend) = &manifest.frontend {
        let bytes = regular(root, "frontend-assets.json", MAX_FRONTEND_INDEX_BYTES)?;
        if hash(&bytes) != frontend.assets_sha256.to_ascii_lowercase() {
            return Err(fail("Frontend index integrity mismatch"));
        }
        let index: FrontendAssetIndex = serde_json::from_slice(&bytes)?;
        index.validate(frontend)?;
        files.push(bound(
            root,
            "frontend-assets.json",
            MAX_FRONTEND_INDEX_BYTES,
        )?);
        for entry in index.files {
            let file = bound(root, &entry.path, 4 * 1024 * 1024)?;
            if file.length != entry.bytes || file.sha256 != entry.sha256.to_ascii_lowercase() {
                return Err(fail("Frontend asset integrity mismatch"));
            }
            files.push(file);
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((manifest, files))
}
pub(crate) fn target(manifest: &PluginManifest) -> Result<String> {
    if [&manifest.id, &manifest.version].iter().any(|s| {
        s.is_empty()
            || s.len() > 160
            || !s
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
            || matches!(s.as_str(), "." | "..")
    }) {
        return Err(fail("Unsafe plugin identity/version"));
    }
    Ok(format!(
        "plugins/{}/{}/package.json",
        manifest.id, manifest.version
    ))
}
pub fn verify_bound_files(package: &Path, files: &[BoundFile]) -> Result<()> {
    if files.len() > 140 {
        return Err(fail("Too many signed files"));
    }
    for file in files {
        if file.length > MAX_COMPONENT_BYTES as u64
            || bound(package, &file.path, file.length as usize)? != *file
        {
            return Err(fail("Approved package bytes changed"));
        }
    }
    Ok(())
}
pub(crate) fn proof_paths(manifest: &PluginManifest) -> Result<Vec<String>> {
    let mut files: Vec<_> = [
        "1.root.json",
        "targets.json",
        "feed.json",
        "snapshot.json",
        "timestamp.json",
    ]
    .into_iter()
    .map(|name| format!("{PROOF_DIR}/metadata/{name}"))
    .collect();
    files.push(format!("{PROOF_DIR}/targets/{}", target(manifest)?));
    Ok(files)
}
pub fn copy_package_proof(source: &Path, destination: &Path) -> Result<()> {
    let (manifest, _) = inventory(source)?;
    for path in proof_paths(&manifest)? {
        let bytes = regular(source, &path, 1024 * 1024)?;
        let output = destination.join(path);
        fs::create_dir_all(
            output
                .parent()
                .ok_or_else(|| fail("Invalid proof destination"))?,
        )?;
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        use std::io::Write;
        options.open(output)?.write_all(&bytes)?;
    }
    Ok(())
}

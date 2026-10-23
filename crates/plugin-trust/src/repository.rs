// SPDX-License-Identifier: MIT
use crate::{
    Result, fail,
    keys::source,
    package::{self, BoundFile, Descriptor, PROOF_DIR},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io::Write, num::NonZeroU64, path::Path};
use tough::{
    FilesystemTransport, IntoVec, RepositoryLoader, TargetName,
    editor::{RepositoryEditor, signed::SignedRole, targets::TargetsEditor},
    schema::KeyHolder,
    schema::{PathPattern, PathSet, RoleKeys, RoleType, Root, Signed, Target},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifiedPackage {
    pub plugin_id: String,
    pub version: String,
    pub root_sha256: String,
    pub manifest_sha256: String,
    pub descriptor_target: String,
    pub metadata_version: u64,
    pub files_verified: usize,
    pub bound_files: Vec<BoundFile>,
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
pub async fn create_root(
    root_key: &Path,
    publisher_key: &Path,
    feed_key: &Path,
    output: &Path,
    expires: jiff::Timestamp,
) -> Result<()> {
    let mut keys = HashMap::new();
    let mut roles = HashMap::new();
    for (role, path) in [
        (RoleType::Root, root_key),
        (RoleType::Targets, publisher_key),
        (RoleType::Snapshot, feed_key),
        (RoleType::Timestamp, feed_key),
    ] {
        let key = source(path)
            .as_sign()
            .await
            .map_err(|_| fail("Cannot load signing key"))?
            .tuf_key();
        let id = key.key_id()?;
        keys.insert(id.clone(), key);
        roles.insert(
            role,
            RoleKeys {
                keyids: vec![id],
                threshold: NonZeroU64::MIN,
                _extra: HashMap::new(),
            },
        );
    }
    let root = Root {
        spec_version: "1.0.0".into(),
        consistent_snapshot: false,
        version: NonZeroU64::MIN,
        expires,
        keys,
        roles,
        _extra: HashMap::new(),
    };
    let signed = SignedRole::new(
        root.clone(),
        &KeyHolder::Root(root),
        &[source(root_key)],
        &aws_lc_rs::rand::SystemRandom::new(),
    )
    .await?;
    write_new(output, signed.buffer())
}
pub async fn sign_package(
    root_path: &Path,
    publisher_key: &Path,
    feed_key: &Path,
    package: &Path,
    version: NonZeroU64,
    expires: jiff::Timestamp,
) -> Result<VerifiedPackage> {
    let (manifest, files) = package::inventory(package)?;
    let name = package::target(&manifest)?;
    let proof = package.join(PROOF_DIR);
    fs::create_dir(&proof)?;
    let target_file = proof.join("targets").join(&name);
    fs::create_dir_all(
        target_file
            .parent()
            .ok_or_else(|| fail("Invalid target path"))?,
    )?;
    write_new(
        &target_file,
        &serde_json::to_vec_pretty(&Descriptor {
            schema_version: 1,
            plugin_id: manifest.id.clone(),
            version: manifest.version.clone(),
            files,
        })?,
    )?;
    let feedkey = source(feed_key)
        .as_sign()
        .await
        .map_err(|_| fail("Cannot load feed key"))?
        .tuf_key();
    let id = feedkey.key_id()?;
    let mut feed = TargetsEditor::new("feed");
    feed.version(version).expires(expires);
    feed.add_target(
        TargetName::new(&name)?,
        Target::from_path(&target_file).await?,
    )?;
    let feed = feed.create_signed(&[source(feed_key)]).await?;
    let root: Signed<Root> = serde_json::from_slice(&fs::read(root_path)?)?;
    let mut top = TargetsEditor::from_targets(
        "targets",
        tough::schema::Targets::new("1.0.0".into(), version, expires),
        KeyHolder::Root(root.signed),
    );
    top.version(version).expires(expires);
    top.delegate_role(
        feed,
        PathSet::Paths(vec![PathPattern::new(format!(
            "plugins/{}/*",
            manifest.id
        ))?]),
        true,
        HashMap::from([(id.clone(), feedkey)]),
        vec![id],
        NonZeroU64::MIN,
    )?;
    let top = top.create_signed(&[source(publisher_key)]).await?;
    let top = Signed {
        signed: top.signed.targets,
        signatures: top.signatures,
    };
    let mut editor = RepositoryEditor::new(root_path).await?;
    editor
        .targets(top)?
        .targets_version(version)?
        .targets_expires(expires)?
        .snapshot_version(version)
        .snapshot_expires(expires)
        .timestamp_version(version)
        .timestamp_expires(expires);
    let signed = editor
        .sign(&[source(publisher_key), source(feed_key)])
        .await?;
    signed.write(proof.join("metadata")).await?;
    verify_package(package, &fs::read(root_path)?).await
}
pub async fn verify_package(package: &Path, trusted_root: &[u8]) -> Result<VerifiedPackage> {
    if trusted_root.len() > 1024 * 1024 {
        return Err(fail("Trusted root exceeds limit"));
    }
    let (manifest, files) = package::inventory(package)?;
    let target = package::target(&manifest)?;
    let paths = package::proof_paths(&manifest)?;
    let proof_files: Vec<_> = paths
        .iter()
        .map(|path| package::bound(package, path, 1024 * 1024))
        .collect::<Result<_>>()?;
    let metadata =
        url::Url::from_directory_path(package.join(PROOF_DIR).join("metadata").canonicalize()?)
            .map_err(|_| fail("Invalid metadata directory"))?;
    let targets =
        url::Url::from_directory_path(package.join(PROOF_DIR).join("targets").canonicalize()?)
            .map_err(|_| fail("Invalid targets directory"))?;
    let repository = RepositoryLoader::new(&trusted_root, metadata, targets)
        .transport(FilesystemTransport)
        .limits(tough::Limits {
            max_root_size: 1024 * 1024,
            max_targets_size: 1024 * 1024,
            max_timestamp_size: 1024 * 1024,
            max_snapshot_size: 1024 * 1024,
            max_root_updates: 1,
        })
        .load()
        .await?;
    // This local package format stores exactly one root; rotation needs a future format.
    if repository.root().signed.version.get() != 1 {
        return Err(fail("Root rotation is unsupported by this package format"));
    }
    let bytes = repository
        .read_target(&TargetName::new(&target)?)
        .await?
        .ok_or_else(|| fail("Signed package target is missing"))?
        .into_vec()
        .await?;
    if bytes.len() > 1024 * 1024 {
        return Err(fail("Signed descriptor exceeds limit"));
    }
    let descriptor: Descriptor = serde_json::from_slice(&bytes)?;
    if descriptor.schema_version != 1
        || descriptor.plugin_id != manifest.id
        || descriptor.version != manifest.version
        || descriptor.files != files
    {
        return Err(fail("Signed package descriptor mismatch"));
    }
    let manifest_sha256 = files
        .iter()
        .find(|file| file.path == "plugin.json")
        .ok_or_else(|| fail("Missing manifest"))?
        .sha256
        .clone();
    let files_verified = files.len();
    let mut bound_files = files;
    bound_files.extend(proof_files);
    Ok(VerifiedPackage {
        plugin_id: manifest.id,
        version: manifest.version,
        root_sha256: package::hash(trusted_root),
        manifest_sha256,
        descriptor_target: target,
        metadata_version: repository.snapshot().signed.version.get(),
        files_verified,
        bound_files,
    })
}
pub fn verify_package_blocking(package: &Path, trusted_root: &[u8]) -> Result<VerifiedPackage> {
    let package = package.to_path_buf();
    let root = trusted_root.to_vec();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?
            .block_on(verify_package(&package, &root))
    })
    .join()
    .map_err(|_| fail("Package verification worker failed"))?
}

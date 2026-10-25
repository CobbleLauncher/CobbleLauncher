// SPDX-License-Identifier: MIT
//! Copies only files matching the original publisher's plan. Foreign URLs,
//! launch arguments, Java installations and credentials are never imported.
use super::*;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions as CapOptions};
use cobble_minecraft::{FileKind, PlannedFile, PlatformContext, VersionCatalog};
use cobble_net::{Broker, BrokerConfig};
use cobble_store::{ArtifactExpectation, ContentStore, ExpectedDigests, IntegrityBasis};
use sha1::{Digest, Sha1};

fn open(root: &Dir, relative: &SafeRelativePath) -> io::Result<cap_std::fs::File> {
    let mut dir = root.try_clone()?;
    let parts = relative.as_str().split('/').collect::<Vec<_>>();
    for part in &parts[..parts.len() - 1] {
        dir = dir.open_dir_nofollow(part)?;
    }
    let mut options = CapOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    dir.open_with(parts[parts.len() - 1], &options)
}
fn reuse_file(
    from: &Dir,
    to: &Dir,
    content: &ContentStore,
    relative: &SafeRelativePath,
    plan: &PlannedFile,
    control: &crate::OperationControl,
) -> Result<bool, Error> {
    control.check_cancelled()?;
    if let Ok(mut existing) = open(to, &plan.path) {
        if existing.metadata()?.len() != plan.size {
            return Ok(false);
        }
        let mut hash = Sha1::new();
        let mut bytes = [0u8; 64 * 1024];
        loop {
            control.check_cancelled()?;
            let count = existing.read(&mut bytes)?;
            if count == 0 {
                break;
            }
            hash.update(&bytes[..count]);
        }
        return Ok(<[u8; 20]>::from(hash.finalize()) == plan.sha1);
    }
    let Ok(mut file) = open(from, relative) else {
        return Ok(false);
    };
    if !file.metadata()?.is_file() || file.metadata()?.len() != plan.size {
        return Ok(false);
    }
    let mut digest = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        control.check_cancelled()?;
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if <[u8; 20]>::from(digest.finalize()) != plan.sha1 {
        return Ok(false);
    }
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(0))?;
    let stored = content.ingest(
        file,
        ArtifactExpectation {
            expected_size: Some(plan.size),
            digests: ExpectedDigests {
                sha1: Some(plan.sha1),
                ..Default::default()
            },
            max_bytes: plan.size,
            basis: IntegrityBasis::TrustedMetadata,
        },
    )?;
    control.check_cancelled()?;
    content.copy_to(&stored.artifact, to, &plan.path)?;
    Ok(true)
}

pub(super) async fn prepare(
    runtime: &LauncherRuntimeSource,
    version: &str,
    target: &Path,
    source: &DownloadSource,
    control: &crate::OperationControl,
) -> Result<(), Error> {
    control.check_cancelled()?;
    let broker = Broker::new(BrokerConfig::default())?;
    let metadata = VersionCatalog::fetch_from(&broker, source)
        .await?
        .fetch_version_from(&broker, version, source)
        .await?;
    let plan = metadata.download_plan(&PlatformContext::current()?)?;
    let runtime = runtime.clone();
    let target = target.to_owned();
    let control = control.clone();
    tokio::task::spawn_blocking(move || -> Result<(), Error> {
        let directory = real_directory(&runtime.directory)?;
        leaf(&runtime.client_version_id)?;
        let from = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
        let to = Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
        let cache = dirs::cache_dir()
            .ok_or("无法定位缓存目录")?
            .join("CobbleLauncher/content");
        fs::create_dir_all(&cache)?;
        let content =
            ContentStore::open(Dir::open_ambient_dir(cache, cap_std::ambient_authority())?)?;
        let mut reused = 0;
        let mut assets = None;
        for file in &plan.files {
            control.check_cancelled()?;
            let alias = SafeRelativePath::parse(&format!(
                "versions/{0}/{0}.jar",
                runtime.client_version_id
            ))?;
            let matched = if file.kind == FileKind::Client {
                reuse_file(&from, &to, &content, &alias, file, &control)?
                    || reuse_file(&from, &to, &content, &file.path, file, &control)?
            } else {
                reuse_file(&from, &to, &content, &file.path, file, &control)?
            };
            reused += usize::from(matched);
            if matched && file.kind == FileKind::AssetIndex {
                let mut bytes = Vec::new();
                open(&to, &file.path)?
                    .take(file.size + 1)
                    .read_to_end(&mut bytes)?;
                let index: Value = serde_json::from_slice(&bytes)?;
                if index["virtual"] != true && index["map_to_resources"] != true {
                    assets = Some(index);
                }
            }
        }
        if let Some(index) = assets {
            let objects = index["objects"].as_object().ok_or("无效的资源索引")?;
            let mut seen = BTreeSet::new();
            for object in objects.values() {
                let hash = object["hash"].as_str().ok_or("资源缺少哈希")?;
                if !seen.insert(hash) {
                    continue;
                }
                if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("资源哈希无效".into());
                }
                let mut sha1 = [0u8; 20];
                for (i, byte) in sha1.iter_mut().enumerate() {
                    *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16)?;
                }
                let file = PlannedFile {
                    path: SafeRelativePath::parse(&format!(
                        "assets/objects/{}/{hash}",
                        &hash[..2]
                    ))?,
                    url: String::new(),
                    size: object["size"].as_u64().ok_or("资源缺少大小")?,
                    sha1,
                    kind: FileKind::Asset,
                };
                reused += usize::from(reuse_file(
                    &from, &to, &content, &file.path, &file, &control,
                )?);
            }
        }
        control.progress()(InstallProgress {
            detail: None,
            phase: "import-runtime".into(),
            completed: reused,
            total: reused,
            reused,
        });
        Ok(())
    })
    .await??;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pcl_runtime_reuse_requires_publisher_hash_and_never_imports_foreign_metadata_or_tokens() {
        let root = std::env::temp_dir().join(format!(
            "cobble-runtime-reuse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for name in ["source/versions/Custom", "target", "cache"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        fs::write(
            root.join("source/versions/Custom/Custom.jar"),
            b"original-client",
        )
        .unwrap();
        fs::write(
            root.join("source/accounts.json"),
            b"do-not-import-credentials",
        )
        .unwrap();
        let from =
            Dir::open_ambient_dir(root.join("source"), cap_std::ambient_authority()).unwrap();
        let to = Dir::open_ambient_dir(root.join("target"), cap_std::ambient_authority()).unwrap();
        let content = ContentStore::open(
            Dir::open_ambient_dir(root.join("cache"), cap_std::ambient_authority()).unwrap(),
        )
        .unwrap();
        let control = crate::OperationControl::new(Arc::new(|_| {}));
        let file = PlannedFile {
            path: SafeRelativePath::parse("versions/1.21.1/1.21.1.jar").unwrap(),
            url: "https://publisher.invalid/never-contacted".into(),
            size: 15,
            sha1: Sha1::digest(b"original-client").into(),
            kind: FileKind::Client,
        };
        let alias = SafeRelativePath::parse("versions/Custom/Custom.jar").unwrap();
        assert!(reuse_file(&from, &to, &content, &alias, &file, &control).unwrap());
        assert!(
            reuse_file(&from, &to, &content, &alias, &file, &control).unwrap(),
            "an interrupted import must reuse its already verified target"
        );
        assert_eq!(
            fs::read(root.join("target/versions/1.21.1/1.21.1.jar")).unwrap(),
            b"original-client"
        );
        assert!(!root.join("target/accounts.json").exists());
        fs::write(
            root.join("source/versions/Custom/Custom.jar"),
            b"different-bytes",
        )
        .unwrap();
        fs::create_dir(root.join("other-target")).unwrap();
        let fresh =
            Dir::open_ambient_dir(root.join("other-target"), cap_std::ambient_authority()).unwrap();
        assert!(!reuse_file(&from, &fresh, &content, &alias, &file, &control).unwrap());
        drop(fresh);
        assert_eq!(
            fs::read(root.join("target/versions/1.21.1/1.21.1.jar")).unwrap(),
            b"original-client"
        );
        drop(content);
        drop(to);
        drop(from);
        fs::remove_dir_all(root).unwrap();
    }
}

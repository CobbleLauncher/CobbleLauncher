// SPDX-License-Identifier: MIT
//! Download a host-selected Modrinth pack into a new local archive, then reuse
//! the existing MRPACK parser before exposing the normal installation preview.
use crate::modrinth::ModDownloads;
use cap_std::fs::Dir;
use cobble_content::{
    Mrpack, PackVersion, read_mrpack, resolve_modrinth_pack, supported_pack_target,
};
use cobble_store::SafeRelativePath;
use std::{fs, path::Path};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn download_modrinth_pack(
    project: &str,
    version: &str,
    destination: &Path,
) -> Result<Mrpack, Error> {
    if destination.try_exists()? {
        return Err("Pack archive destination already exists".into());
    }
    download_modrinth_pack_controlled(
        project,
        version,
        destination,
        crate::OperationControl::new(std::sync::Arc::new(|_| {})),
    )
    .await
}

pub async fn download_modrinth_pack_controlled(
    project: &str,
    version: &str,
    destination: &Path,
    control: crate::OperationControl,
) -> Result<Mrpack, Error> {
    control.check_cancelled()?;
    let progress = control.progress();
    progress(crate::InstallProgress {
        detail: None,
        phase: "metadata".into(),
        completed: 0,
        total: 1,
        reused: 0,
    });

    let parent = destination
        .parent()
        .ok_or("Pack archive destination has no parent")?;
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Pack archive filename must be UTF-8")?;
    let name = SafeRelativePath::parse(name)?;
    let selection = resolve_modrinth_pack(project, version).await?;
    control.check_cancelled()?;
    if destination.try_exists()? {
        let path = destination.to_owned();
        return tokio::task::spawn_blocking(move || {
            if !crate::modrinth::matches_downloaded_file(&path, &selection.file)? { return Err("Existing pack archive differs from the selected download; it was not overwritten".into()); }
            validated_pack(&path, &selection.version)
        }).await?;
    }
    let downloads = ModDownloads::open()?;
    progress(crate::InstallProgress {
        detail: None,
        phase: "pack-download".into(),
        completed: 0,
        total: 1,
        reused: 0,
    });
    let artifact = downloads
        .fetch_file_controlled(&selection.file, &control)
        .await?;
    let parent = parent.to_owned();
    let destination = destination.to_owned();
    tokio::task::spawn_blocking(move || {
        control.check_cancelled()?;
        progress(crate::InstallProgress {
            detail: None,
            phase: "pack-install".into(),
            completed: 0,
            total: 1,
            reused: 0,
        });
        let root = Dir::open_ambient_dir(parent, cap_std::ambient_authority())?;
        // CAS publication refuses existing destinations. Cleanup below therefore
        // applies only after this call has successfully created the archive.
        downloads.content.copy_to(&artifact, &root, &name)?;
        let result = validated_pack(&destination, &selection.version);
        if result.is_err() {
            let _ = fs::remove_file(&destination);
        }
        if result.is_ok() {
            progress(crate::InstallProgress {
                detail: None,
                phase: "complete".into(),
                completed: 1,
                total: 1,
                reused: 0,
            });
        }
        result
    })
    .await?
}

fn validated_pack(path: &Path, selected: &PackVersion) -> Result<Mrpack, Error> {
    let pack = read_mrpack(path)?;
    let loader = pack.loader_kind.as_deref().unwrap_or("minecraft");
    if !selected
        .minecraft_versions
        .contains(&pack.minecraft_version)
        || !selected.loaders.iter().any(|kind| kind == loader)
        || !supported_pack_target(&pack.minecraft_version, loader)
    {
        return Err(
            "Downloaded MRPACK Minecraft version or loader disagrees with the selected release"
                .into(),
        );
    }
    Ok(pack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-pack-download-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn archive(&self, mc: &str, loader: Option<&str>) -> PathBuf {
            let path = self.0.join("test.mrpack");
            let mut dependencies = serde_json::Map::new();
            dependencies.insert("minecraft".into(), mc.into());
            if let Some(loader) = loader {
                dependencies.insert(loader.into(), "0.1.0".into());
            }
            let index = serde_json::json!({"formatVersion":1,"game":"minecraft","name":"Synthetic pack","versionId":"1.0","files":[],"dependencies":dependencies});
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            zip.start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(&serde_json::to_vec(&index).unwrap()).unwrap();
            zip.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn selected() -> PackVersion {
        PackVersion {
            version_id: "opaque-version".into(),
            name: "Pack 1.0".into(),
            version_number: "1.0".into(),
            minecraft_versions: vec!["1.21.1".into()],
            loaders: vec!["fabric".into()],
            version_type: "release".into(),
            date_published: "2026-10-22T00:00:00Z".into(),
        }
    }
    #[test]
    fn downloaded_manifest_must_match_actual_supported_game_and_loader() {
        let fixture = Fixture::new();
        assert!(
            validated_pack(
                &fixture.archive("1.21.1", Some("fabric-loader")),
                &selected()
            )
            .is_ok()
        );
        assert!(
            validated_pack(
                &fixture.archive("1.20.1", Some("fabric-loader")),
                &selected()
            )
            .is_err()
        );
        assert!(validated_pack(&fixture.archive("1.21.1", None), &selected()).is_err());
        let mut vanilla = selected();
        vanilla.loaders = vec!["minecraft".into()];
        assert!(validated_pack(&fixture.archive("1.21.1", None), &vanilla).is_ok());
        let mut unsupported = selected();
        unsupported.minecraft_versions = vec!["1.16.5".into()];
        assert!(
            validated_pack(
                &fixture.archive("1.16.5", Some("fabric-loader")),
                &unsupported
            )
            .is_err()
        );
    }
    #[tokio::test]
    async fn existing_archive_is_rejected_before_network_or_replacement() {
        let fixture = Fixture::new();
        let path = fixture.0.join("keep.mrpack");
        fs::write(&path, b"user archive").unwrap();
        assert!(
            download_modrinth_pack("Invalid project!", "version", &path)
                .await
                .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), b"user archive");
    }
}

// SPDX-License-Identifier: MIT
//! Download verified Modrinth ZIPs, then reuse the local pack importer.
//! Caller holds the stopped-instance write lease. Packs are never activated here.
use crate::modrinth::ModDownloads;
use cap_std::fs::Dir;
use cobble_content::{LocalPack, LocalPackKind, import_local_packs, resolve_modrinth_content_pack};
use cobble_store::{ContentStore, SafeRelativePath, VerifiedArtifact};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn install_modrinth_content_pack(
    directory: &Path,
    kind: LocalPackKind,
    mc: &str,
    project_id: &str,
    version_id: &str,
) -> Result<Vec<LocalPack>, Error> {
    let selected = resolve_modrinth_content_pack(kind, mc, project_id, version_id).await?;
    let downloads = ModDownloads::open()?;
    let artifact = downloads.fetch_file(&selected.file).await?;
    let directory = directory.to_owned();
    tokio::task::spawn_blocking(move || {
        import_artifact(
            &directory,
            kind,
            &downloads.content,
            &artifact,
            &selected.file.filename,
        )
    })
    .await?
}

fn staging_directory() -> Result<PathBuf, Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    for _ in 0..16 {
        let path = std::env::temp_dir().join(format!(
            "cobble-content-pack-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("无法创建内容包临时目录，请重试".into())
}

fn import_artifact(
    directory: &Path,
    kind: LocalPackKind,
    content: &ContentStore,
    artifact: &VerifiedArtifact,
    filename: &str,
) -> Result<Vec<LocalPack>, Error> {
    let name = SafeRelativePath::parse(filename)?;
    if name.as_str().contains('/') {
        return Err("内容包文件名无效".into());
    }
    let staging = staging_directory()?;
    let archive = staging.join(filename);
    let result = (|| -> Result<Vec<LocalPack>, Error> {
        let root = Dir::open_ambient_dir(&staging, cap_std::ambient_authority())?;
        content.copy_to(artifact, &root, &name)?;
        import_local_packs(directory, kind, std::slice::from_ref(&archive))
    })();
    // Only this operation's private ZIP and empty temporary directory are removed.
    let _ = fs::remove_file(archive);
    let _ = fs::remove_dir(staging);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_store::{ArtifactExpectation, ExpectedDigests, IntegrityBasis};
    use std::io::{Cursor, Write};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = staging_directory().unwrap();
            fs::create_dir(path.join("content")).unwrap();
            fs::create_dir(path.join("game")).unwrap();
            Self(path)
        }
        fn store(&self) -> ContentStore {
            ContentStore::open(
                Dir::open_ambient_dir(self.0.join("content"), cap_std::ambient_authority())
                    .unwrap(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn artifact(content: &ContentStore, shader: bool) -> (VerifiedArtifact, Vec<u8>) {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(
                if shader {
                    "shaders/program.fsh"
                } else {
                    "pack.mcmeta"
                },
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive
            .write_all(if shader {
                b"void main() {}"
            } else {
                br#"{"pack":{"pack_format":34,"description":"Fixture pack"}}"#
            })
            .unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let artifact = content
            .ingest(
                bytes.as_slice(),
                ArtifactExpectation {
                    expected_size: Some(bytes.len() as u64),
                    digests: ExpectedDigests::default(),
                    max_bytes: bytes.len() as u64,
                    basis: IntegrityBasis::UserConfirmedLocal,
                },
            )
            .unwrap()
            .artifact;
        (artifact, bytes)
    }

    #[test]
    fn cached_zip_uses_local_import_validation_and_keeps_selection_unchanged() {
        let fixture = Fixture::new();
        let content = fixture.store();
        let (resource, original) = artifact(&content, false);
        let directory = fixture.0.join("game");
        fs::write(directory.join("options.txt"), b"resourcePacks:[]").unwrap();
        let packs = import_artifact(
            &directory,
            LocalPackKind::ResourcePack,
            &content,
            &resource,
            "Faithful fixture.zip",
        )
        .unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].description.as_deref(), Some("Fixture pack"));
        assert_eq!(
            fs::read(directory.join("resourcepacks/Faithful fixture.zip")).unwrap(),
            original
        );
        assert_eq!(
            fs::read(directory.join("options.txt")).unwrap(),
            b"resourcePacks:[]"
        );
        assert!(
            import_artifact(
                &directory,
                LocalPackKind::ResourcePack,
                &content,
                &resource,
                "Faithful fixture.zip"
            )
            .is_err()
        );
        assert!(
            import_artifact(
                &directory,
                LocalPackKind::ShaderPack,
                &content,
                &resource,
                "wrong-kind.zip"
            )
            .is_err()
        );
        assert!(!directory.join("shaderpacks/wrong-kind.zip").exists());
        let (shader, bytes) = artifact(&content, true);
        let packs = import_artifact(
            &directory,
            LocalPackKind::ShaderPack,
            &content,
            &shader,
            "MakeUp fixture.zip",
        )
        .unwrap();
        assert_eq!(packs[0].file_name, "MakeUp fixture.zip");
        assert_eq!(
            fs::read(directory.join("shaderpacks/MakeUp fixture.zip")).unwrap(),
            bytes
        );
    }

    #[test]
    fn existing_foreign_files_and_traversal_names_are_never_overwritten() {
        let fixture = Fixture::new();
        let content = fixture.store();
        let (resource, _) = artifact(&content, false);
        let directory = fixture.0.join("game");
        fs::create_dir(directory.join("resourcepacks")).unwrap();
        fs::write(directory.join("resourcepacks/Pack.zip"), b"user file").unwrap();
        assert!(
            import_artifact(
                &directory,
                LocalPackKind::ResourcePack,
                &content,
                &resource,
                "pack.zip"
            )
            .is_err()
        );
        assert!(
            import_artifact(
                &directory,
                LocalPackKind::ResourcePack,
                &content,
                &resource,
                "../escaped.zip"
            )
            .is_err()
        );
        assert_eq!(
            fs::read(directory.join("resourcepacks/Pack.zip")).unwrap(),
            b"user file"
        );
        assert!(!fixture.0.join("escaped.zip").exists());
    }
}

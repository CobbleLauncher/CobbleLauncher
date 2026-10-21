// SPDX-License-Identifier: MIT
//! Original content and fault fixtures; no third-party game content.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cap_std::{ambient_authority, fs::Dir};
use cobble_store::{
    ArtifactExpectation, CasError, ContentStore, ExpectedDigests, IntegrityBasis, SafeRelativePath,
};
use sha2::{Digest, Sha256};

struct Temp(PathBuf);
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cobble-cas-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn dir(&self) -> Dir {
        open_root(&self.0)
    }
    fn store(&self) -> ContentStore {
        ContentStore::open(self.dir()).unwrap()
    }
    fn parts(&self) -> PathBuf {
        self.0.join("sha256/.parts")
    }
    fn assert_no_parts(&self) {
        assert_eq!(std::fs::read_dir(self.parts()).unwrap().count(), 0);
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn open_root(path: &Path) -> Dir {
    Dir::open_ambient_dir(path, ambient_authority()).unwrap()
}
fn local(max_bytes: u64) -> ArtifactExpectation {
    ArtifactExpectation {
        expected_size: None,
        digests: ExpectedDigests::default(),
        max_bytes,
        basis: IntegrityBasis::UserConfirmedLocal,
    }
}
fn digest<const N: usize>(value: &str) -> [u8; N] {
    assert_eq!(value.len(), N * 2);
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}
fn abc() -> ArtifactExpectation {
    // Published standard hash-vector bytes; the fixture itself is just ASCII abc.
    ArtifactExpectation {
        expected_size: Some(3),
        max_bytes: 3,
        basis: IntegrityBasis::TrustedMetadata,
        digests: ExpectedDigests {
            sha256: Some(digest(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            )),
            sha1: Some(digest("a9993e364706816aba3e25717850c26c9cd0d89d")),
            sha512: Some(digest(
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
            )),
        },
    }
}
fn cas_path(root: &Temp, hex: &str) -> PathBuf {
    root.0.join("sha256").join(&hex[..2]).join(hex)
}

#[test]
fn store002_all_hashes_verified_dedup_and_restart_reopen() {
    let root = Temp::new();
    let first = root.store().ingest(&b"abc"[..], abc()).unwrap();
    assert!(!first.reused);
    assert_eq!(first.artifact.size_bytes(), 3);
    assert_eq!(first.artifact.expected_digests(), abc().digests);
    assert_eq!(
        first.artifact.integrity_basis(),
        IntegrityBasis::TrustedMetadata
    );
    let second = root.store().ingest(&b"abc"[..], abc()).unwrap();
    assert!(second.reused);
    assert_eq!(first.artifact, second.artifact);
    let reopened = root
        .store()
        .open_verified(first.artifact.sha256(), abc())
        .unwrap();
    assert_eq!(reopened, first.artifact);
    assert_eq!(
        std::fs::read(cas_path(&root, &reopened.sha256_hex())).unwrap(),
        b"abc"
    );
    root.assert_no_parts();
}

#[test]
fn store002_each_wrong_hash_and_size_refuses_commit() {
    let root = Temp::new();
    let store = root.store();
    for index in 0..4 {
        let mut expected = abc();
        match index {
            0 => expected.digests.sha256.as_mut().unwrap()[0] ^= 1,
            1 => expected.digests.sha1.as_mut().unwrap()[0] ^= 1,
            2 => expected.digests.sha512.as_mut().unwrap()[0] ^= 1,
            _ => expected.expected_size = Some(2),
        }
        assert!(matches!(
            store.ingest(&b"abc"[..], expected),
            Err(CasError::DigestMismatch(_) | CasError::SizeMismatch)
        ));
        root.assert_no_parts();
        assert!(
            !cas_path(
                &root,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            )
            .exists()
        );
    }
    assert!(matches!(
        store.ingest(
            &b"abc"[..],
            ArtifactExpectation {
                basis: IntegrityBasis::TrustedMetadata,
                ..local(3)
            }
        ),
        Err(CasError::InvalidExpectation)
    ));
    assert!(matches!(
        store.ingest(&b"abc"[..], local(u64::MAX)),
        Err(CasError::InvalidExpectation)
    ));
    assert!(matches!(
        store.ingest(io::repeat(0), local(1000)),
        Err(CasError::Limit)
    ));
    root.assert_no_parts();
}

struct FailingRead {
    first: bool,
}
impl Read for FailingRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.first {
            return Err(io::Error::other("synthetic connection loss"));
        }
        self.first = true;
        let count = buffer.len().min(1024);
        buffer[..count].fill(b'x');
        Ok(count)
    }
}

#[test]
fn store002_interrupted_ingestion_cleans_its_part() {
    let root = Temp::new();
    let store = root.store();
    assert!(matches!(
        store.ingest(FailingRead { first: false }, local(4096)),
        Err(CasError::Io(_))
    ));
    root.assert_no_parts();
    assert_eq!(std::fs::read_dir(root.0.join("sha256")).unwrap().count(), 2);
}

#[test]
fn store002_concurrent_same_digest_publish_is_no_clobber() {
    let root = Temp::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let path = root.0.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let store = ContentStore::open(open_root(&path)).unwrap();
                barrier.wait();
                store.ingest(&b"abc"[..], abc()).unwrap()
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| !result.reused).count(), 1);
    assert!(
        results
            .iter()
            .all(|result| result.artifact == results[0].artifact)
    );
    root.assert_no_parts();
}

#[test]
fn store002_corrupt_cas_is_never_reused_overwritten_or_copied() {
    let root = Temp::new();
    let store = root.store();
    let artifact = store.ingest(&b"abc"[..], abc()).unwrap().artifact;
    let path = cas_path(&root, &artifact.sha256_hex());
    std::fs::write(&path, b"xyz").unwrap();
    assert!(matches!(
        store.ingest(&b"abc"[..], abc()),
        Err(CasError::CorruptExisting)
    ));
    assert!(matches!(
        store.open_verified(artifact.sha256(), abc()),
        Err(CasError::CorruptExisting)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"xyz");
    let instance = Temp::new();
    assert!(matches!(
        store.copy_to(
            &artifact,
            &instance.dir(),
            &SafeRelativePath::parse("mods/a.jar").unwrap()
        ),
        Err(CasError::CorruptExisting)
    ));
    assert_eq!(
        std::fs::read_dir(instance.0.join("mods")).unwrap().count(),
        0
    );
    root.assert_no_parts();
    assert!(matches!(
        store.open_verified([0; 32], local(10)),
        Err(CasError::NotFound)
    ));
}

#[test]
fn store003_mutable_instances_are_independent_copies() {
    let root = Temp::new();
    let store = root.store();
    let artifact = store.ingest(&b"abc"[..], abc()).unwrap().artifact;
    let a = Temp::new();
    let b = Temp::new();
    let target = SafeRelativePath::parse("nested/mods/a.jar").unwrap();
    store.copy_to(&artifact, &a.dir(), &target).unwrap();
    store.copy_to(&artifact, &b.dir(), &target).unwrap();
    std::fs::write(a.0.join(target.as_str()), b"user changed mod").unwrap();
    assert_eq!(std::fs::read(b.0.join(target.as_str())).unwrap(), b"abc");
    assert_eq!(
        std::fs::read(cas_path(&root, &artifact.sha256_hex())).unwrap(),
        b"abc"
    );
    assert!(store.copy_to(&artifact, &a.dir(), &target).is_err());
    assert_eq!(
        std::fs::read(a.0.join(target.as_str())).unwrap(),
        b"user changed mod"
    );
    assert_eq!(
        std::fs::read_dir(a.0.join("nested/mods")).unwrap().count(),
        1
    );
}

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
struct PausedRead {
    first: bool,
    ready: PathBuf,
}
impl Read for PausedRead {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.first {
            self.first = true;
            let count = buffer.len().min(1024);
            buffer[..count].fill(b'x');
            return Ok(count);
        }
        std::fs::write(&self.ready, b"part written; shared lifecycle lease held")?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        Err(io::Error::other(
            "parent did not kill child before deadline",
        ))
    }
}

#[test]
fn store002_crossprocess_live_writer_and_crash_cleanup() {
    const ENV: &str = "COBBLE_CAS_TEST_CHILD_ROOT";
    if let Some(root) = std::env::var_os(ENV) {
        let root = PathBuf::from(root);
        let store = ContentStore::open(open_root(&root)).unwrap();
        let _ = store.ingest(
            PausedRead {
                first: false,
                ready: root.join("writer-ready"),
            },
            local(4096),
        );
        panic!("fault child should have been killed by the parent");
    }
    let root = Temp::new();
    let mut store = root.store();
    std::fs::write(
        root.parts().join("keep-unrecognized.part"),
        b"not owned by scoped cleanup",
    )
    .unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "store002_crossprocess_live_writer_and_crash_cleanup",
            "--nocapture",
        ])
        .env(ENV, &root.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut child = ChildGuard(Some(child));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !root.0.join("writer-ready").exists() {
        assert!(Instant::now() < deadline, "child readiness timed out");
        assert!(
            child.0.as_mut().unwrap().try_wait().unwrap().is_none(),
            "child exited before readiness"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(matches!(
        store.cleanup_stale_parts_exclusive(),
        Err(CasError::Busy)
    ));
    let parts: Vec<_> = std::fs::read_dir(root.parts())
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
    assert_eq!(parts.len(), 2);
    assert!(parts.iter().any(
        |entry| entry.file_name().to_string_lossy().starts_with("cas-")
            && std::fs::read(entry.path()).unwrap().len() == 1024
    ));
    let mut process = child.0.take().unwrap();
    process.kill().unwrap();
    assert!(!process.wait().unwrap().success());
    drop(store);
    let mut reopened = root.store();
    assert_eq!(reopened.cleanup_stale_parts_exclusive().unwrap(), 1);
    assert_eq!(std::fs::read_dir(root.parts()).unwrap().count(), 1);
    assert_eq!(
        std::fs::read(root.parts().join("keep-unrecognized.part")).unwrap(),
        b"not owned by scoped cleanup"
    );
}

fn redirect(link: &Path, target: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    {
        let output = Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(link.components().collect::<PathBuf>())
            .arg(target.components().collect::<PathBuf>())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction creation failed: {} -> {}: {} {}",
            link.display(),
            target.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn remove_redirect(link: &Path) {
    #[cfg(unix)]
    std::fs::remove_file(link).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(link).unwrap();
}

#[test]
fn store002_and_store003_reparse_roots_prefixes_and_destinations_refused() {
    let outside = Temp::new();
    let root = Temp::new();
    redirect(&root.0.join("sha256"), &outside.0);
    assert!(ContentStore::open(root.dir()).is_err());
    remove_redirect(&root.0.join("sha256"));
    let store = root.store();
    redirect(&root.0.join("sha256/ba"), &outside.0);
    assert!(store.ingest(&b"abc"[..], abc()).is_err());
    assert_eq!(std::fs::read_dir(&outside.0).unwrap().count(), 0);
    root.assert_no_parts();
    remove_redirect(&root.0.join("sha256/ba"));
    let artifact = store.ingest(&b"abc"[..], abc()).unwrap().artifact;
    let instance = Temp::new();
    redirect(&instance.0.join("mods"), &outside.0);
    assert!(
        store
            .copy_to(
                &artifact,
                &instance.dir(),
                &SafeRelativePath::parse("mods/a.jar").unwrap()
            )
            .is_err()
    );
    assert_eq!(std::fs::read_dir(&outside.0).unwrap().count(), 0);
    remove_redirect(&instance.0.join("mods"));
}

#[test]
fn store002_empty_local_content_and_cleanup_nonregular_rejection() {
    let root = Temp::new();
    let mut store = root.store();
    let artifact = store.ingest(io::empty(), local(0)).unwrap().artifact;
    assert_eq!(
        artifact.sha256_hex(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        artifact.integrity_basis(),
        IntegrityBasis::UserConfirmedLocal
    );
    let outside = Temp::new();
    std::fs::write(outside.0.join("precious"), b"untouched").unwrap();
    let link = root
        .parts()
        .join("cas-0123456789abcdef0123456789abcdef.part");
    redirect(&link, &outside.0);
    assert!(store.cleanup_stale_parts_exclusive().is_err());
    assert_eq!(
        std::fs::read(outside.0.join("precious")).unwrap(),
        b"untouched"
    );
    remove_redirect(&link);
}

#[test]
fn local_content_sha256_is_computed_not_a_provider_identity() {
    let root = Temp::new();
    let artifact = root
        .store()
        .ingest(&b"original synthetic payload"[..], local(1024))
        .unwrap()
        .artifact;
    assert_eq!(
        artifact.sha256(),
        <[u8; 32]>::from(Sha256::digest(b"original synthetic payload"))
    );
    assert_eq!(artifact.expected_digests(), ExpectedDigests::default());
    assert_eq!(
        artifact.integrity_basis(),
        IntegrityBasis::UserConfirmedLocal
    );
}

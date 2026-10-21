// SPDX-License-Identifier: MIT
//! Optional SHA-1 -> SHA-256 hints. Only the CAS's full verification authorizes reuse.
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_store::{ArtifactExpectation, CasError, ContentStore, VerifiedArtifact};
use std::io::{self, Read, Write};

type Error = Box<dyn std::error::Error + Send + Sync>;

pub fn lookup(
    content: &ContentStore,
    index: &Dir,
    expected: ArtifactExpectation,
) -> Result<Option<VerifiedArtifact>, Error> {
    let Some(sha1) = expected.digests.sha1 else {
        return Ok(None);
    };
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    let file = match index.open_with(hex(&sha1), &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::with_capacity(65);
    file.take(65).read_to_end(&mut bytes)?;
    if bytes.len() != 64 || !bytes.iter().all(u8::is_ascii_hexdigit) {
        return Ok(None);
    }
    let mut sha256 = [0; 32];
    for (byte, pair) in sha256.iter_mut().zip(bytes.as_chunks::<2>().0) {
        // The ASCII check above guarantees UTF-8 and hexadecimal digits.
        *byte = u8::from_str_radix(std::str::from_utf8(pair)?, 16)?;
    }
    match content.open_verified(sha256, expected) {
        Ok(artifact) => Ok(Some(artifact)),
        Err(CasError::NotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Best effort performance hint: callers may ignore a failed recording after
/// successful artifact publication. Existing hints are never overwritten.
pub fn record(index: &Dir, artifact: &VerifiedArtifact) -> Result<(), Error> {
    let Some(sha1) = artifact.expected_digests().sha1 else {
        return Ok(());
    };
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut file = match index.open_with(hex(&sha1), &options) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    file.write_all(artifact.sha256_hex().as_bytes())?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 15) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use cobble_store::{ExpectedDigests, IntegrityBasis};
    use sha1::{Digest, Sha1};
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cobble-cache-{}-{nonce}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::create_dir(path.join("content")).unwrap();
            std::fs::create_dir(path.join("index")).unwrap();
            Self(path)
        }
        fn index(&self) -> Dir {
            self.dir("index")
        }
        fn dir(&self, name: &str) -> Dir {
            Dir::open_ambient_dir(self.0.join(name), cap_std::ambient_authority()).unwrap()
        }
        fn content(&self) -> ContentStore {
            ContentStore::open(self.dir("content")).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn expected() -> ArtifactExpectation {
        ArtifactExpectation {
            expected_size: Some(3),
            max_bytes: 3,
            basis: IntegrityBasis::TrustedMetadata,
            digests: ExpectedDigests {
                sha1: Some(Sha1::digest(b"abc").into()),
                ..ExpectedDigests::default()
            },
        }
    }

    #[test]
    fn hint_reuses_real_cas_only_after_full_expected_verification() {
        let temp = Temp::new();
        let content = temp.content();
        let index = temp.index();
        let artifact = content
            .ingest(b"abc".as_slice(), expected())
            .unwrap()
            .artifact;
        record(&index, &artifact).unwrap();
        record(&index, &artifact).unwrap();
        assert_eq!(
            index
                .read(hex(&expected().digests.sha1.unwrap()))
                .unwrap()
                .len(),
            64
        );
        let found = lookup(&content, &index, expected()).unwrap().unwrap();
        assert_eq!(found.sha256(), artifact.sha256());
        assert_eq!(found.size_bytes(), artifact.size_bytes());
        assert_eq!(found.expected_digests().sha1, expected().digests.sha1);
        let mut wrong_size = expected();
        wrong_size.expected_size = Some(2);
        assert!(lookup(&content, &index, wrong_size).is_err());
        let mut wrong_hash = expected();
        wrong_hash.digests.sha512 = Some([0; 64]);
        assert!(lookup(&content, &index, wrong_hash).is_err());
    }

    #[test]
    fn missing_stale_and_malformed_hints_never_become_cache_success() {
        let temp = Temp::new();
        let content = temp.content();
        let index = temp.index();
        let name = hex(&expected().digests.sha1.unwrap());
        assert!(lookup(&content, &index, expected()).unwrap().is_none());
        index.write(&name, b"malformed").unwrap();
        assert!(lookup(&content, &index, expected()).unwrap().is_none());
        index.write(&name, "0".repeat(64).as_bytes()).unwrap();
        assert!(lookup(&content, &index, expected()).unwrap().is_none());
        let artifact = content
            .ingest(b"abc".as_slice(), expected())
            .unwrap()
            .artifact;
        index
            .write(&name, artifact.sha256_hex().as_bytes())
            .unwrap();
        let digest = artifact.sha256_hex();
        temp.dir("content")
            .write(format!("sha256/{}/{digest}", &digest[..2]), b"bad")
            .unwrap();
        assert!(lookup(&content, &index, expected()).is_err());
    }
}

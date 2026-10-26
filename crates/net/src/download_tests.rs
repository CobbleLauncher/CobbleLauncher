// SPDX-License-Identifier: MIT
use super::*;
use crate::tests::{FixtureRouting, Server, config, raw, root};
use crate::{CredentialScope, ProviderCredentials};
use cap_std::fs::Dir;
use sha2::Sha512;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

const URL: &str = "https://one.fixture.invalid/file";
const BODY: &[u8] = b"abcdef";

#[test]
fn adoptium_requests_accept_verified_zip_and_tar_packages_only() {
    let mut expectation = expected();
    expectation.digests.sha256 = Some([1; 32]);
    for filename in ["runtime.zip", "runtime.tar.gz"] {
        let url = format!(
            "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B1/{filename}"
        );
        let request = DownloadRequest::new(
            &url,
            expectation,
            DownloadAuthority::same_origin(
                "adoptium",
                "managed-java",
                "temurin-ga-archive-v1",
                None,
            )
            .unwrap(),
            None,
        )
        .unwrap();
        assert!(request.with_adoptium_redirects().is_ok(), "{filename}");
    }
    for filename in ["runtime.exe", "runtime.tar.gz.exe", "runtime.tgz"] {
        let url = format!(
            "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.1%2B1/{filename}"
        );
        let request = DownloadRequest::new(
            &url,
            expectation,
            DownloadAuthority::same_origin(
                "adoptium",
                "managed-java",
                "temurin-ga-archive-v1",
                None,
            )
            .unwrap(),
            None,
        )
        .unwrap();
        assert!(request.with_adoptium_redirects().is_err(), "{filename}");
    }
}

#[test]
fn curseforge_redirect_request_identity_is_explicit_and_cannot_mix_mirror_grants() {
    let mut expectation = expected();
    expectation.digests.sha1 = Some([0; 20]);
    let request = DownloadRequest::new(
        "https://edge.forgecdn.net/files/1/a.jar",
        expectation,
        DownloadAuthority::same_origin("curseforge", "install", "official-v1", None).unwrap(),
        None,
    )
    .unwrap();
    let granted = request.clone().with_curseforge_redirects().unwrap();
    assert_ne!(request.key(), granted.key());
    assert!(granted.clone().with_curseforge_redirects().is_err());
    assert!(
        granted
            .clone()
            .with_mirror_redirects(MIRROR_ORIGINAL)
            .is_err()
    );
    assert!(granted.with_modpack_redirects().is_err());
    for url in [
        "https://evil.invalid/files/1/a.jar",
        "https://edge.forgecdn.net/files/1/a.jar?key=synthetic",
        "https://edge.forgecdn.net/other/a.jar",
    ] {
        assert!(
            DownloadRequest::new(
                url,
                expectation,
                DownloadAuthority::same_origin("curseforge", "install", "official-v1", None)
                    .unwrap(),
                None
            )
            .unwrap()
            .with_curseforge_redirects()
            .is_err()
        );
    }
}

#[test]
fn modpack_redirects_are_limited_to_documented_anonymous_download_hosts() {
    for url in [
        "https://cdn.modrinth.com/data/test/versions/one/mod.jar",
        "https://github.com/example/mod/releases/download/one/mod.jar",
        "https://raw.githubusercontent.com/example/mod/main/mod.jar",
        "https://gitlab.com/example/mod/-/releases/one/downloads/mod.jar",
    ] {
        let plain = DownloadRequest::new(
            url,
            expected(),
            DownloadAuthority::same_origin("mrpack", "install", "mrpack-v1", None).unwrap(),
            None,
        )
        .unwrap();
        let granted = plain.clone().with_modpack_redirects().unwrap();
        assert_ne!(plain.key(), granted.key());
        assert!(granted.with_modpack_redirects().is_err());
    }
    let request = DownloadRequest::new(
        URL,
        expected(),
        DownloadAuthority::same_origin("mrpack", "install", "mrpack-v1", None).unwrap(),
        None,
    )
    .unwrap();
    assert!(request.with_modpack_redirects().is_err());
}

const MIRROR_SHA1: &str = "1f8ac10f23c5b5bc1167bda84b833e5c057a77d2";
const MIRROR_ORIGINAL: &str = "https://piston-data.mojang.com/v1/objects/fixture/client.jar";
fn mirror_request() -> DownloadRequest {
    let mut expected = expected();
    let mut sha1 = [0; 20];
    for (index, byte) in sha1.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&MIRROR_SHA1[index * 2..index * 2 + 2], 16).unwrap();
    }
    expected.digests.sha1 = Some(sha1);
    DownloadRequest::new(
        URL,
        expected,
        DownloadAuthority::same_origin("fixture-mirror", "artifact", "mirror-v1", None).unwrap(),
        None,
    )
    .unwrap()
}

#[tokio::test]
async fn mirror_download_follows_provider_cdn_and_storage_routes_and_verifies_all_digests() {
    let server = Server::new(|input| {
        if input.starts_with("GET /file ") {
            raw(format!("HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/download/{MIRROR_SHA1}?signature=synthetic\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"))
        } else if input.to_ascii_lowercase().contains("host: two.fixture.invalid") {
            raw(format!("HTTP/1.1 302 Found\r\nLocation: https://one.fixture.invalid/bmclapi/1f/{MIRROR_SHA1}?signature=storage\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"))
        } else { raw(full()) }
    }).await;
    let temp = Temp::new();
    let plain = mirror_request();
    let granted = plain
        .clone()
        .with_mirror_redirects(MIRROR_ORIGINAL)
        .unwrap();
    assert_ne!(plain.key(), granted.key());
    let result = temp
        .downloader(server.broker(config()))
        .download(granted, TransferCancellation::default())
        .await
        .unwrap();
    assert_eq!(result.artifact.size_bytes(), 6);
    assert_eq!(server.captured().len(), 3);
}

#[tokio::test]
async fn mirror_download_rejects_incorrect_bytes_after_provider_redirect() {
    let server = Server::new(|input| if input.to_ascii_lowercase().contains("host: one.fixture.invalid") {
        raw(b"HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/storage/file.jar\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    } else { raw(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nwrong!") }).await;
    let temp = Temp::new();
    assert!(matches!(
        temp.downloader(server.broker(config()))
            .download(
                mirror_request()
                    .with_mirror_redirects(MIRROR_ORIGINAL)
                    .unwrap(),
                TransferCancellation::default()
            )
            .await,
        Err(DownloadError::Integrity(CasError::DigestMismatch(_)))
    ));
    assert_eq!(server.captured().len(), 2);
}

#[tokio::test]
async fn mirror_redirects_still_reject_non_public_destinations() {
    let server = Server::new(|_| raw(b"HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1/storage/file.jar\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")).await;
    let temp = Temp::new();
    assert!(matches!(
        temp.downloader(server.broker(config()))
            .download(
                mirror_request()
                    .with_mirror_redirects(MIRROR_ORIGINAL)
                    .unwrap(),
                TransferCancellation::default()
            )
            .await,
        Err(DownloadError::Transfer(TransferError::Network(
            NetError::ForbiddenDestination
        )))
    ));
    assert_eq!(server.captured().len(), 1);
}

#[tokio::test]
async fn mirror_redirect_policy_requires_anonymous_sha1_and_official_original() {
    assert!(request().with_mirror_redirects(MIRROR_ORIGINAL).is_err());
    assert!(
        mirror_request()
            .with_mirror_redirects("https://third-party.invalid/file")
            .is_err()
    );
    let server = Server::new(|_| raw(full())).await;
    let broker = server.broker(config());
    let partition = CredentialPartition::new("fixture:mirror-auth").unwrap();
    let credential = broker
        .register_download_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/").unwrap(),
            ProviderCredentials {
                bearer: Some("synthetic".into()),
                api_key: None,
                cookie: None,
            },
            partition,
        )
        .unwrap();
    let anonymous = mirror_request();
    let authenticated = DownloadRequest::new(
        URL,
        anonymous.expectation,
        DownloadAuthority::same_origin("fixture-mirror", "artifact", "mirror-v1", Some(partition))
            .unwrap(),
        Some(credential),
    )
    .unwrap();
    assert!(
        authenticated
            .with_mirror_redirects(MIRROR_ORIGINAL)
            .is_err()
    );
    assert!(server.captured().is_empty());
}

#[cfg(test)]
mod managed {
    use super::*;
    use crate::{DownloadManager, ManagedDownloadError};

    #[tokio::test]
    async fn manager_bounds_queued_jobs_and_subscribers_and_drop_closes_subscription() {
        use std::future::Future;
        let server = Server::new(|_| raw(full())).await;
        let temp = Temp::new();
        let mut downloader = temp.downloader(server.broker(config()));
        downloader.jobs = Arc::new(Semaphore::new(0));
        let manager = DownloadManager::new(downloader);
        let mut pending = Vec::new();
        for _ in 0..64 {
            let mut future = Box::pin(manager.download(request(), TransferCancellation::default()));
            std::future::poll_fn(|cx| {
                assert!(future.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            pending.push(future);
        }
        assert!(matches!(
            manager
                .download(request(), TransferCancellation::default())
                .await,
            Err(ManagedDownloadError::Busy)
        ));
        // Dropping each future invokes its subscription guard. The last one
        // leaves a closing entry until the spawned worker observes cancellation.
        drop(pending);
        assert!(matches!(
            manager
                .download(request(), TransferCancellation::default())
                .await,
            Err(ManagedDownloadError::Busy)
        ));
        let mut pending = Vec::new();
        for index in 0..15 {
            let request = DownloadRequest::new(
                &format!("https://one.fixture.invalid/{index}"),
                expected(),
                DownloadAuthority::same_origin("p", "artifact", "policy", None).unwrap(),
                None,
            )
            .unwrap();
            let mut future = Box::pin(manager.download(request, TransferCancellation::default()));
            std::future::poll_fn(|cx| {
                assert!(future.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            pending.push(future);
        }
        let extra = DownloadRequest::new(
            "https://one.fixture.invalid/extra",
            expected(),
            DownloadAuthority::same_origin("p", "artifact", "policy", None).unwrap(),
            None,
        )
        .unwrap();
        assert!(matches!(
            manager
                .download(extra, TransferCancellation::default())
                .await,
            Err(ManagedDownloadError::Busy)
        ));
        assert!(server.captured().is_empty());
    }

    #[tokio::test]
    async fn manager_checks_credential_namespace_before_joining_identical_job() {
        let server = Server::new(|_| vec![(Duration::from_millis(100), full())]).await;
        let broker = server.broker(config());
        let other_broker = server.broker(config());
        let partition = CredentialPartition::new("fixture:principal").unwrap();
        let register = |broker: &Broker| {
            broker
                .register_download_credentials(
                    CredentialScope::new("https://one.fixture.invalid", "/").unwrap(),
                    ProviderCredentials {
                        bearer: None,
                        api_key: Some("synthetic-key".into()),
                        cookie: None,
                    },
                    partition,
                )
                .unwrap()
        };
        let valid = register(&broker);
        let foreign = register(&other_broker);
        let make_request = |credential| {
            DownloadRequest::new(
                URL,
                expected(),
                DownloadAuthority::same_origin("p", "artifact", "policy", Some(partition)).unwrap(),
                Some(credential),
            )
            .unwrap()
        };
        let (valid, foreign) = (make_request(valid), make_request(foreign));
        assert_eq!(valid.key(), foreign.key());
        let temp = Temp::new();
        let manager = DownloadManager::new(temp.downloader(broker));
        let (ok, denied) = tokio::join!(
            manager.download(valid, TransferCancellation::default()),
            manager.download(foreign, TransferCancellation::default()),
        );
        assert!(ok.is_ok());
        assert!(matches!(denied, Err(ManagedDownloadError::Download(error))
            if matches!(*error, DownloadError::Policy(NetError::CredentialScope))));
        assert_eq!(server.captured().len(), 1);
    }

    #[tokio::test]
    async fn manager_two_subscribers_share_one_verified_transfer() {
        let server = Server::new(|_| vec![(Duration::from_millis(100), full())]).await;
        let temp = Temp::new();
        let manager =
            DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
        let (a, b) = tokio::join!(
            manager.download(request(), TransferCancellation::default()),
            manager.download(request(), TransferCancellation::default()),
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.artifact.size_bytes(), 6);
        assert_eq!(server.captured().len(), 1);
    }

    #[tokio::test]
    async fn manager_cancel_one_subscriber_preserves_the_other() {
        let server = Server::new(|_| vec![(Duration::from_millis(150), full())]).await;
        let temp = Temp::new();
        let manager =
            DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
        let cancel = TransferCancellation::default();
        let (a, b, _) = tokio::join!(
            manager.download(request(), cancel.clone()),
            manager.download(request(), TransferCancellation::default()),
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                cancel.cancel();
            },
        );
        assert!(matches!(a, Err(ManagedDownloadError::Cancelled)));
        assert!(b.is_ok());
        assert_eq!(server.captured().len(), 1);
    }

    #[tokio::test]
    async fn manager_last_subscriber_cancel_keeps_writer_closed_until_worker_exit() {
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let server = Server::new(move |_| {
            if observed.fetch_add(1, Ordering::Relaxed) == 0 {
                vec![
                    (Duration::ZERO, interrupted()),
                    (Duration::from_secs(5), b"def".to_vec()),
                ]
            } else {
                raw(suffix())
            }
        })
        .await;
        let temp = Temp::new();
        let manager =
            DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
        let cancel = TransferCancellation::default();
        let running_manager = manager.clone();
        let running_cancel = cancel.clone();
        let running =
            tokio::spawn(async move { running_manager.download(request(), running_cancel).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !std::fs::metadata(temp.part(request().identity)).is_ok_and(|m| m.len() == 3) {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        cancel.cancel();
        assert!(matches!(
            running.await.unwrap(),
            Err(ManagedDownloadError::Cancelled)
        ));
        tokio::time::timeout(Duration::from_secs(3), manager.wait_idle())
            .await
            .unwrap();
        // `wait_idle` includes the real curl/checkpoint owner, not only the
        // cancelled subscriber. Its staging write lease is available now.
        drop(temp.stage().open_job(request().identity).unwrap());
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                match manager
                    .download(request(), TransferCancellation::default())
                    .await
                {
                    Err(ManagedDownloadError::Busy) => {
                        tokio::time::sleep(Duration::from_millis(10)).await
                    }
                    other => break other,
                }
            }
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result.artifact.size_bytes(), 6);
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn manager_retry_after_and_attempt_limit_and_no_auth_retry() {
        for (response, expected_calls) in [
            (
                b"HTTP/1.1 503 Unavailable\r\nRetry-After: 0\r\nContent-Length: 0\r\n\r\n"
                    .as_slice(),
                3,
            ),
            (
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n".as_slice(),
                1,
            ),
            (
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".as_slice(),
                1,
            ),
        ] {
            let server = Server::new(move |_| raw(response)).await;
            let temp = Temp::new();
            let manager =
                DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
            assert!(
                manager
                    .download(request(), TransferCancellation::default())
                    .await
                    .is_err()
            );
            assert_eq!(server.captured().len(), expected_calls);
        }
        let count = Arc::new(AtomicUsize::new(0));
        let observed = count.clone();
        let server = Server::new(move |_| {
            raw(if observed.fetch_add(1, Ordering::Relaxed) == 0 {
                b"HTTP/1.1 503 Unavailable\r\nRetry-After: 1\r\nContent-Length: 0\r\n\r\n".to_vec()
            } else {
                full()
            })
        })
        .await;
        let temp = Temp::new();
        let manager =
            DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
        let start = Instant::now();
        assert!(
            manager
                .download(request(), TransferCancellation::default())
                .await
                .is_ok()
        );
        assert!(start.elapsed() >= Duration::from_secs(1));
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn manager_retry_after_larger_than_total_budget_stops_without_second_attempt() {
        let server = Server::new(|_| {
            raw(b"HTTP/1.1 429 Limited\r\nRetry-After: 20\r\nContent-Length: 0\r\n\r\n")
        })
        .await;
        let temp = Temp::new();
        let manager =
            DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
        assert!(
            matches!(manager.download(request(), TransferCancellation::default()).await,
            Err(ManagedDownloadError::Download(error)) if matches!(*error, DownloadError::Deadline))
        );
        assert_eq!(server.captured().len(), 1);
    }
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        for _ in 0..64 {
            let path = std::env::temp_dir().join(format!(
                "cobble-download-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            if std::fs::create_dir(&path).is_ok() {
                std::fs::create_dir(path.join("downloads")).unwrap();
                std::fs::create_dir(path.join("content")).unwrap();
                return Self(path);
            }
        }
        panic!("test directory allocation failed");
    }
    fn stage(&self) -> DownloadStaging {
        DownloadStaging::open(
            Dir::open_ambient_dir(self.0.join("downloads"), cap_std::ambient_authority()).unwrap(),
        )
        .unwrap()
    }
    fn cas(&self) -> ContentStore {
        ContentStore::open(
            Dir::open_ambient_dir(self.0.join("content"), cap_std::ambient_authority()).unwrap(),
        )
        .unwrap()
    }
    fn downloader(&self, broker: Broker) -> ArtifactDownloader {
        ArtifactDownloader::new(broker, self.stage(), self.cas())
    }
    fn part(&self, identity: StageIdentity) -> PathBuf {
        self.0
            .join("downloads/jobs")
            .join(hex(&identity.request_key))
            .join("partial.part")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn expected() -> ArtifactExpectation {
    ArtifactExpectation {
        expected_size: Some(6),
        max_bytes: 6,
        basis: IntegrityBasis::TrustedMetadata,
        digests: ExpectedDigests {
            sha256: Some(Sha256::digest(BODY).into()),
            sha512: Some(Sha512::digest(BODY).into()),
            sha1: None,
        },
    }
}
fn request() -> DownloadRequest {
    DownloadRequest::new(
        URL,
        expected(),
        DownloadAuthority::same_origin(
            "synthetic-provider",
            "artifact-test",
            "explicit-test-grant",
            None,
        )
        .unwrap(),
        None,
    )
    .unwrap()
}
fn full() -> Vec<u8> {
    b"HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef"
        .to_vec()
}
fn suffix() -> Vec<u8> {
    b"HTTP/1.1 206 Partial Content\r\nETag: \"v1\"\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef".to_vec()
}
fn interrupted() -> Vec<u8> {
    b"HTTP/1.1 200 OK\r\nETag: \"v1\"\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc".to_vec()
}
fn resume(stage: &DownloadStage, offset: u64) {
    let StageRecovery::Resume(cp) = stage.recovery() else {
        panic!("durable resume expected");
    };
    assert_eq!(cp.durable_offset, offset);
    assert_eq!(cp.validator.as_deref(), Some(b"\"v1\"".as_slice()));
}
fn assert_short_body_failure(result: Result<DownloadResult, DownloadError>) {
    assert!(
        matches!(
            result,
            Err(DownloadError::Transfer(
                TransferError::Network(NetError::Protocol) | TransferError::SizeMismatch
            ))
        ),
        "fixture must reach the truncated body, got {:?}",
        result.err()
    );
}
async fn wait_stage(temp: &Temp, identity: StageIdentity) -> DownloadStage {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            match temp.stage().open_job(identity) {
                Ok(stage) => return stage,
                Err(StageError::Busy) => tokio::time::sleep(Duration::from_millis(10)).await,
                Err(error) => panic!("stage recovery failed: {error}"),
            }
        }
    })
    .await
    .expect("stage worker did not exit")
}

#[tokio::test]
async fn download_first_full_interruption_persists_real_etag_then_reopens_206_into_verified_cas() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let server = Server::new(move |_| {
        raw(if observed.fetch_add(1, Ordering::Relaxed) == 0 {
            interrupted()
        } else {
            suffix()
        })
    })
    .await;
    let temp = Temp::new();
    let identity = request().identity;
    let downloader = temp.downloader(server.broker(config()));
    assert_short_body_failure(
        downloader
            .download(request(), TransferCancellation::default())
            .await,
    );
    let stage = temp.stage().open_job(identity).unwrap();
    resume(&stage, 3);
    drop(stage);
    assert!(matches!(
        temp.cas()
            .open_verified(Sha256::digest(BODY).into(), expected()),
        Err(CasError::NotFound)
    ));
    let result = temp
        .downloader(server.broker(config()))
        .download(request(), TransferCancellation::default())
        .await
        .unwrap();
    assert_eq!(result.artifact.size_bytes(), 6);
    assert_eq!(result.artifact.sha256(), Sha256::digest(BODY).as_slice());
    assert!(!result.staging_retained);
    assert!(!temp.part(identity).exists());
    let captures = server.captured();
    assert_eq!(captures.len(), 2);
    assert!(!captures[0].to_ascii_lowercase().contains("range:"));
    assert!(captures[1].to_ascii_lowercase().contains("range: bytes=3-"));
    assert!(
        captures[1]
            .to_ascii_lowercase()
            .contains("if-range: \"v1\"")
    );
    temp.cas()
        .open_verified(result.artifact.sha256(), expected())
        .unwrap();
}

#[tokio::test]
async fn download_200_resets_partial_and_changed_206_restarts_once_as_full() {
    for changed in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = calls.clone();
        let server = Server::new(move |_| raw(match observed.fetch_add(1, Ordering::Relaxed) {
            0 => interrupted(),
            1 if changed => b"HTTP/1.1 206 Partial Content\r\nETag: \"v2\"\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef".to_vec(),
            _ => full(),
        })).await;
        let temp = Temp::new();
        let downloader = temp.downloader(server.broker(config()));
        assert_short_body_failure(
            downloader
                .download(request(), TransferCancellation::default())
                .await,
        );
        let result = downloader
            .download(request(), TransferCancellation::default())
            .await
            .unwrap();
        assert_eq!(result.artifact.sha256(), Sha256::digest(BODY).as_slice());
        let captures = server.captured();
        assert_eq!(captures.len(), if changed { 3 } else { 2 });
        assert!(captures[1].to_ascii_lowercase().contains("range: bytes=3-"));
        if changed {
            assert!(!captures[2].to_ascii_lowercase().contains("range:"));
        }
    }
}

#[tokio::test]
async fn download_416_rehashes_whole_file_and_never_blesses_corrupt_prefix_or_wrong_hash() {
    for valid in [true, false] {
        let server = Server::new(|_| raw(b"HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 0\r\nContent-Range: bytes */6\r\nConnection: close\r\n\r\n".to_vec())).await;
        let temp = Temp::new();
        let identity = request().identity;
        let mut stage = temp.stage().open_job(identity).unwrap();
        stage
            .write_all(if valid { BODY } else { b"abcXYZ" })
            .unwrap();
        stage.checkpoint(Some(b"\"v1\"")).unwrap();
        drop(stage);
        let result = temp
            .downloader(server.broker(config()))
            .download(request(), TransferCancellation::default())
            .await;
        if valid {
            assert_eq!(
                result.unwrap().artifact.sha256(),
                Sha256::digest(BODY).as_slice()
            );
        } else {
            assert!(matches!(
                result,
                Err(DownloadError::Integrity(CasError::DigestMismatch(_)))
            ));
            assert!(temp.part(identity).exists());
            assert!(matches!(
                temp.cas()
                    .open_verified(Sha256::digest(BODY).into(), expected()),
                Err(CasError::NotFound)
            ));
        }
        assert!(
            server.captured()[0]
                .to_ascii_lowercase()
                .contains("range: bytes=6-")
        );
    }
    let server = Server::new(|_| raw(full())).await;
    for algorithm in ["sha1", "sha512"] {
        let temp = Temp::new();
        let mut want = expected();
        if algorithm == "sha1" {
            want.digests.sha1 = Some([0; 20]);
        } else {
            want.digests.sha512 = Some([0; 64]);
        }
        let input = DownloadRequest::new(
            URL,
            want,
            DownloadAuthority::same_origin("p", "a", "policy", None).unwrap(),
            None,
        )
        .unwrap();
        assert!(matches!(
            temp.downloader(server.broker(config()))
                .download(input, TransferCancellation::default())
                .await,
            Err(DownloadError::Integrity(CasError::DigestMismatch(_)))
        ));
        assert!(matches!(
            temp.cas()
                .open_verified(Sha256::digest(BODY).into(), expected()),
            Err(CasError::NotFound)
        ));
    }
}

#[tokio::test]
async fn download_source_grant_and_registered_credential_partition_are_independent_public_ip_gates()
{
    let server = Server::new(|_| raw(b"HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/file\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())).await;
    let broker = server.broker(config());
    let partition = CredentialPartition::new("synthetic-provider:principal-record-a").unwrap();
    let other = CredentialPartition::new("synthetic-provider:principal-record-b").unwrap();
    let credential = broker
        .register_download_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/").unwrap(),
            ProviderCredentials {
                bearer: None,
                api_key: Some("synthetic-private-key".into()),
                cookie: None,
            },
            partition,
        )
        .unwrap();
    let temp = Temp::new();
    let downloader = temp.downloader(broker.clone());
    let wrong = DownloadRequest::new(
        URL,
        expected(),
        DownloadAuthority::same_origin("p", "a", "policy", Some(other)).unwrap(),
        Some(credential.clone()),
    )
    .unwrap();
    assert!(matches!(
        downloader
            .download(wrong, TransferCancellation::default())
            .await,
        Err(DownloadError::Policy(NetError::CredentialScope))
    ));
    assert!(server.captured().is_empty());
    let input = DownloadRequest::new(
        URL,
        expected(),
        DownloadAuthority::same_origin("p", "a", "policy", Some(partition)).unwrap(),
        Some(credential.clone()),
    )
    .unwrap();
    let identity = input.identity;
    assert!(matches!(
        downloader
            .download(input, TransferCancellation::default())
            .await,
        Err(DownloadError::Transfer(TransferError::SourceNotAuthorized))
    ));
    assert_eq!(server.captured().len(), 1);
    assert!(server.captured()[0].contains("synthetic-private-key"));
    assert_eq!(std::fs::metadata(temp.part(identity)).unwrap().len(), 0);
    let different = server.broker(config());
    let input = DownloadRequest::new(
        URL,
        expected(),
        DownloadAuthority::same_origin("p", "a", "policy", Some(partition)).unwrap(),
        Some(credential),
    )
    .unwrap();
    assert!(matches!(
        temp.downloader(different)
            .download(input, TransferCancellation::default())
            .await,
        Err(DownloadError::Policy(NetError::CredentialScope))
    ));
    assert_eq!(server.captured().len(), 1);
    assert!(
        broker
            .register_download_credentials(
                CredentialScope::new("https://one.fixture.invalid", "/").unwrap(),
                ProviderCredentials {
                    bearer: None,
                    api_key: Some("other-synthetic-key".into()),
                    cookie: None
                },
                partition
            )
            .is_err()
    );
}

#[test]
fn download_canonical_key_binds_all_content_authority_and_exact_source_fields() {
    let baseline = request().identity.request_key;
    for field in 0..8 {
        let mut want = expected();
        let (mut provider, mut purpose, mut policy) =
            ("synthetic-provider", "artifact-test", "explicit-test-grant");
        let mut url = URL;
        match field {
            0 => want.digests.sha1 = Some([9; 20]),
            1 => want.digests.sha256 = Some([9; 32]),
            2 => want.digests.sha512 = None,
            3 => want.max_bytes = 7,
            4 => provider = "other",
            5 => purpose = "other",
            6 => policy = "other",
            _ => url = "https://one.fixture.invalid/file?different=1",
        }
        let input = DownloadRequest::new(
            url,
            want,
            DownloadAuthority::same_origin(provider, purpose, policy, None).unwrap(),
            None,
        )
        .unwrap();
        assert_ne!(input.identity.request_key, baseline);
    }
    let mut want = expected();
    want.expected_size = Some(5);
    assert_ne!(
        DownloadRequest::new(
            URL,
            want,
            DownloadAuthority::same_origin(
                "synthetic-provider",
                "artifact-test",
                "explicit-test-grant",
                None
            )
            .unwrap(),
            None
        )
        .unwrap()
        .identity
        .request_key,
        baseline
    );
    want = expected();
    want.basis = IntegrityBasis::UserConfirmedLocal;
    assert!(
        DownloadRequest::new(
            URL,
            want,
            DownloadAuthority::same_origin("p", "a", "policy", None).unwrap(),
            None
        )
        .is_err()
    );
    want = expected();
    want.digests = ExpectedDigests::default();
    assert!(
        DownloadRequest::new(
            URL,
            want,
            DownloadAuthority::same_origin("p", "a", "policy", None).unwrap(),
            None
        )
        .is_err()
    );
}

#[tokio::test]
async fn download_cancelled_first_full_retains_job_until_checkpoint_and_later_resumes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let server = Server::new(move |_| {
        if observed.fetch_add(1, Ordering::Relaxed) == 0 {
            vec![
                (Duration::ZERO, interrupted()),
                (Duration::from_secs(10), b"def".to_vec()),
            ]
        } else {
            raw(suffix())
        }
    })
    .await;
    let temp = Temp::new();
    let identity = request().identity;
    let downloader = temp.downloader(server.broker(config()));
    let token = TransferCancellation::default();
    let child_token = token.clone();
    let child_downloader = downloader.clone();
    let mut running =
        tokio::spawn(async move { child_downloader.download(request(), child_token).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !std::fs::metadata(temp.part(identity)).is_ok_and(|m| m.len() == 3) {
            if running.is_finished() {
                let result = (&mut running).await.unwrap().map(|_| ());
                panic!(
                    "transfer ended before observable prefix: {result:?}; requests={}",
                    server.captured().len()
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        temp.stage().open_job(identity),
        Err(StageError::Busy)
    ));
    token.cancel();
    assert!(matches!(
        running.await.unwrap(),
        Err(DownloadError::Cancelled)
    ));
    let stage = wait_stage(&temp, identity).await;
    resume(&stage, 3);
    drop(stage);
    assert!(matches!(
        temp.cas()
            .open_verified(Sha256::digest(BODY).into(), expected()),
        Err(CasError::NotFound)
    ));
    assert_eq!(
        downloader
            .download(request(), TransferCancellation::default())
            .await
            .unwrap()
            .artifact
            .size_bytes(),
        6
    );
}

#[test]
fn download_ambiguous_cas_publication_requires_actual_bounded_reopen_confirmation() {
    let temp = Temp::new();
    let cas = temp.cas();
    let digest: [u8; 32] = Sha256::digest(BODY).into();
    let budget = StageRecoveryBudget::new(
        Instant::now() + Duration::from_secs(1),
        Arc::new(AtomicBool::new(false)),
    );
    let ambiguous = || {
        Err(CasError::PublicationMayExist(io::Error::other(
            "synthetic directory sync failure",
        )))
    };
    assert!(matches!(
        confirm_publication(&cas, ambiguous(), digest, expected(), &budget),
        Err(DownloadError::Integrity(CasError::PublicationMayExist(_)))
    ));
    cas.ingest(BODY, expected()).unwrap();
    assert_eq!(
        confirm_publication(&cas, ambiguous(), digest, expected(), &budget)
            .unwrap()
            .artifact
            .sha256(),
        digest
    );
    let stopped = StageRecoveryBudget::new(
        Instant::now() + Duration::from_secs(1),
        Arc::new(AtomicBool::new(true)),
    );
    assert!(confirm_publication(&cas, ambiguous(), digest, expected(), &stopped).is_err());
    cas.open_verified(digest, expected()).unwrap();
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
#[tokio::test]
async fn download_child() {
    let Some(path) = std::env::var_os("COBBLE_DOWNLOAD_CHILD_ROOT") else {
        return;
    };
    let temp = std::mem::ManuallyDrop::new(Temp(PathBuf::from(path)));
    let address = std::env::var("COBBLE_DOWNLOAD_CHILD_ADDRESS")
        .unwrap()
        .parse()
        .unwrap();
    let broker = Broker::fixture(config(), FixtureRouting::new(address), Some(root()));
    temp.downloader(broker)
        .download(request(), TransferCancellation::default())
        .await
        .unwrap();
    panic!("child fault point not reached");
}
#[tokio::test]
async fn download_real_child_crash_before_and_after_cas_preserves_recoverable_stage() {
    let server = Server::new(|_| raw(full())).await;
    for phase in ["before_cas", "after_cas"] {
        let temp = Temp::new();
        let signal = temp.0.join("child-ready");
        let mut child = OwnedChild(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "download::tests::download_child", "--nocapture"])
                .env("COBBLE_DOWNLOAD_CHILD_ROOT", &temp.0)
                .env("COBBLE_DOWNLOAD_CHILD_ADDRESS", server.address.to_string())
                .env("COBBLE_DOWNLOAD_CHILD_PHASE", phase)
                .env("COBBLE_DOWNLOAD_CHILD_SIGNAL", &signal)
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        tokio::time::timeout(Duration::from_secs(10), async {
            while !signal.exists() {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "child exited before fault point"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("owned child fault deadline");
        assert!(matches!(
            temp.stage().open_job(request().identity),
            Err(StageError::Busy)
        ));
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        let stage = temp.stage().open_job(request().identity).unwrap();
        resume(&stage, 6);
        drop(stage);
        let result = temp
            .cas()
            .open_verified(Sha256::digest(BODY).into(), expected());
        if phase == "after_cas" {
            assert_eq!(result.unwrap().size_bytes(), 6);
        } else {
            assert!(matches!(result, Err(CasError::NotFound)));
        }
        assert_eq!(std::fs::read(temp.part(request().identity)).unwrap(), BODY);
    }
}

#[tokio::test]
async fn download_real_range_to_200_reset_crash_cannot_reuse_old_prefix() {
    let server = Server::new(|_| raw(full())).await;
    let temp = Temp::new();
    let mut stage = temp.stage().open_job(request().identity).unwrap();
    stage.write_all(b"abc").unwrap();
    stage.checkpoint(Some(b"\"v1\"")).unwrap();
    drop(stage);
    let signal = temp.0.join("child-reset-ready");
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "download::tests::download_child", "--nocapture"])
            .env("COBBLE_DOWNLOAD_CHILD_ROOT", &temp.0)
            .env("COBBLE_DOWNLOAD_CHILD_ADDRESS", server.address.to_string())
            .env("COBBLE_DOWNLOAD_CHILD_PHASE", "after_full_reset")
            .env("COBBLE_DOWNLOAD_CHILD_SIGNAL", &signal)
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while !signal.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited before header/reset fault"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert!(
        server.captured()[0]
            .to_ascii_lowercase()
            .contains("range: bytes=3-")
    );
    assert!(matches!(
        temp.stage().open_job(request().identity),
        Err(StageError::Busy)
    ));
    assert_eq!(
        std::fs::metadata(temp.part(request().identity))
            .unwrap()
            .len(),
        0
    );
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let stage = temp.stage().open_job(request().identity).unwrap();
    assert!(matches!(stage.recovery(), StageRecovery::Full));
    assert_eq!(stage.byte_len().unwrap(), 0);
    drop(stage);
    let result = temp
        .downloader(server.broker(config()))
        .download(request(), TransferCancellation::default())
        .await
        .unwrap();
    assert_eq!(result.artifact.size_bytes(), 6);
    assert_eq!(server.captured().len(), 2);
    assert!(!server.captured()[1].to_ascii_lowercase().contains("range:"));
}

#[tokio::test]
async fn download_explicit_checkpoint_publication_failure_cannot_enter_cas() {
    let server = Server::new(|_| {
        vec![
            (Duration::ZERO, interrupted()),
            (Duration::from_millis(500), b"def".to_vec()),
        ]
    })
    .await;
    let temp = Temp::new();
    let identity = request().identity;
    let mut settings = config();
    settings.no_progress_timeout = Duration::from_secs(2);
    let downloader = temp.downloader(server.broker(settings));
    let mut running = tokio::spawn(async move {
        downloader
            .download(request(), TransferCancellation::default())
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !std::fs::metadata(temp.part(identity)).is_ok_and(|m| m.len() == 3) {
            if running.is_finished() {
                let result = (&mut running).await.unwrap().map(|_| ());
                panic!(
                    "transfer ended before checkpoint-failure injection: {result:?}; requests={}",
                    server.captured().len()
                );
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    // Header acceptance really completed before the synthetic failure: its
    // zero-offset snapshot exists and the first body prefix is on the inode.
    // This is an unsafe-leaf publication failure, not a simulated disk-full or
    // FlushFileBuffers failure. The publisher must refuse the directory.
    let snapshot = temp.part(identity).with_file_name("checkpoint.sqlite");
    std::fs::remove_file(&snapshot).unwrap();
    std::fs::create_dir(&snapshot).unwrap();
    assert!(matches!(
        running.await.unwrap(),
        Err(DownloadError::Staging(_))
    ));
    assert_eq!(std::fs::read(temp.part(identity)).unwrap(), BODY);
    assert!(matches!(
        temp.cas()
            .open_verified(Sha256::digest(BODY).into(), expected()),
        Err(CasError::NotFound)
    ));
}

#[tokio::test]
async fn adaptive_download_admission_grows_on_throughput_and_backs_off_on_503() {
    use crate::DownloadManager;
    let throttles = Arc::new(AtomicUsize::new(0));
    let observed = throttles.clone();
    let server = Server::new(move |input| {
        if input.starts_with("GET /throttle ") && observed.fetch_add(1, Ordering::Relaxed) == 0 {
            vec![(Duration::ZERO, b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())]
        } else { vec![(Duration::from_millis(100), full())] }
    }).await;
    let temp = Temp::new();
    let manager = DownloadManager::with_adaptive(temp.downloader(server.broker(config())), true);
    assert_eq!(manager.current_concurrency(), 2);
    let request_for = |path: &str| {
        DownloadRequest::new(
            &format!("https://one.fixture.invalid/{path}"),
            expected(),
            DownloadAuthority::same_origin("fixture", "adaptive", "local-v1", None).unwrap(),
            None,
        )
        .unwrap()
    };
    let mut workers = tokio::task::JoinSet::new();
    let started = Instant::now();
    for index in 0..12 {
        let manager = manager.clone();
        let request = request_for(&format!("warm-{index}"));
        workers.spawn(async move {
            manager
                .download(request, TransferCancellation::default())
                .await
                .unwrap();
        });
    }
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    let raised = manager.current_concurrency();
    let warm_elapsed = started.elapsed();
    let warm_bytes = manager.metrics().bytes_received;
    assert_eq!(
        server.captured().len(),
        12,
        "warm-up must perform real local HTTPS transfers"
    );
    assert!(
        raised > 2 && raised <= manager.concurrency(),
        "measured transfers should grow within ceiling: {raised}"
    );
    let manager_copy = manager.clone();
    let request = request_for("throttle");
    let throttled_started = Instant::now();
    let job = tokio::spawn(async move {
        manager_copy
            .download(request, TransferCancellation::default())
            .await
            .unwrap();
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while manager.current_concurrency() >= raised {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("503 should reduce admission before retry");
    let lowered = manager.current_concurrency();
    assert_eq!(
        throttles.load(Ordering::Relaxed),
        1,
        "Retry-After must still delay the retry"
    );
    job.await.unwrap();
    assert!(throttled_started.elapsed() >= Duration::from_secs(1));
    manager.wait_idle().await;
    eprintln!(
        "adaptive local HTTPS: initial=2 raised={raised} reduced={lowered} warm_bytes={warm_bytes} warm_ms={} retry_after_ms={}",
        warm_elapsed.as_millis(),
        throttled_started.elapsed().as_millis()
    );
}

#[tokio::test]
async fn adaptive_download_manual_limit_survives_429_retry() {
    use crate::DownloadManager;
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let server = Server::new(move |_| {
        if observed.fetch_add(1, Ordering::Relaxed) == 0 { raw(b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()) }
        else { raw(full()) }
    }).await;
    let temp = Temp::new();
    let mut settings = config();
    settings.max_concurrent = 3;
    settings.per_origin = 1;
    let manager = DownloadManager::with_adaptive(temp.downloader(server.broker(settings)), false);
    let started = Instant::now();
    manager
        .download(request(), TransferCancellation::default())
        .await
        .unwrap();
    assert_eq!(manager.current_concurrency(), 3);
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert!(started.elapsed() >= Duration::from_secs(1));
    manager.wait_idle().await;
    attempts.store(0, Ordering::Relaxed);
    let automatic_temp = Temp::new();
    let automatic =
        DownloadManager::with_adaptive(automatic_temp.downloader(server.broker(settings)), true);
    let automatic_copy = automatic.clone();
    let automatic_job = tokio::spawn(async move {
        automatic_copy
            .download(request(), TransferCancellation::default())
            .await
            .unwrap();
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while automatic.current_concurrency() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        automatic.current_concurrency(),
        1,
        "automatic mode must react to the first 429, even when its retry succeeds"
    );
    automatic_job.await.unwrap();
    automatic.wait_idle().await;
}

#[tokio::test]
async fn adaptive_download_timeout_reduces_limit_and_cancelled_requests_stay_idle() {
    use crate::DownloadManager;
    let server = Server::new(|_| vec![(Duration::from_millis(300), full())]).await;
    let temp = Temp::new();
    let mut settings = config();
    settings.no_progress_timeout = Duration::from_millis(50);
    let manager = DownloadManager::with_adaptive(temp.downloader(server.broker(settings)), true);
    assert!(
        manager
            .download(request(), TransferCancellation::default())
            .await
            .is_err()
    );
    assert_eq!(manager.current_concurrency(), 1);
    manager.wait_idle().await;
    let cancel = TransferCancellation::default();
    cancel.cancel();
    assert!(matches!(
        manager.download(request(), cancel).await,
        Err(crate::ManagedDownloadError::Cancelled)
    ));
    assert_eq!(manager.metrics().active_downloads, 0);
}

#[tokio::test]
async fn download_pipeline_multi_host_metrics_and_local_benchmark() {
    use crate::DownloadManager;
    let attempts = Arc::new(AtomicUsize::new(0));
    let observed = attempts.clone();
    let server = Server::new(move |input| {
        if input.starts_with("GET /retry ") && observed.fetch_add(1,Ordering::Relaxed)==0 {
            vec![(Duration::from_millis(40),b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())]
        } else { vec![(Duration::from_millis(100), full())] }
    }).await;
    let temp = Temp::new();
    let baseline_temp = Temp::new();
    let mut capped = baseline_temp.downloader(server.broker(config()));
    // Reproduce the previous two-transfer cap without changing disk semantics.
    capped.jobs = Arc::new(Semaphore::new(2));
    let baseline_manager = DownloadManager::with_adaptive(capped, false);
    let manager = DownloadManager::with_adaptive(temp.downloader(server.broker(config())), false);
    let make_request = |url: &str| {
        DownloadRequest::new(
            url,
            expected(),
            DownloadAuthority::same_origin("fixture", "benchmark", "local-v1", None).unwrap(),
            None,
        )
        .unwrap()
    };
    let urls = [
        "https://one.fixture.invalid/a",
        "https://one.fixture.invalid/b",
        "https://two.fixture.invalid/a",
        "https://two.fixture.invalid/b",
    ];
    let baseline_started = Instant::now();
    let mut baseline_workers = tokio::task::JoinSet::new();
    for url in urls {
        let manager = baseline_manager.clone();
        let request = make_request(url);
        baseline_workers.spawn(async move {
            manager
                .download(request, TransferCancellation::default())
                .await
                .unwrap()
        });
    }
    while let Some(result) = baseline_workers.join_next().await {
        result.unwrap();
    }
    let previous_cap = baseline_started.elapsed();
    let started = Instant::now();
    let mut workers = tokio::task::JoinSet::new();
    for url in urls {
        let manager = manager.clone();
        let request = make_request(url);
        workers.spawn(async move {
            manager
                .download(request, TransferCancellation::default())
                .await
                .unwrap()
        });
    }
    tokio::time::sleep(Duration::from_millis(65)).await;
    let live = manager.metrics();
    assert!(
        live.active_downloads >= 3,
        "disk recovery lease must not cap socket concurrency to two: {}",
        live.active_downloads
    );
    assert!(
        live.items
            .iter()
            .all(|item| item.source == "fixture" && !item.item.contains('?'))
    );
    while let Some(result) = workers.join_next().await {
        result.unwrap();
    }
    let cold = started.elapsed();
    assert_eq!(manager.metrics().bytes_received, 24);
    assert_eq!(manager.metrics().active_downloads, 0);
    let started = Instant::now();
    let cached = temp
        .cas()
        .open_verified(Sha256::digest(BODY).into(), expected())
        .unwrap();
    assert_eq!(cached.size_bytes(), 6);
    let cached_time = started.elapsed();
    let started = Instant::now();
    manager
        .download(
            make_request("https://one.fixture.invalid/retry"),
            TransferCancellation::default(),
        )
        .await
        .unwrap();
    let retry = started.elapsed();
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    assert_eq!(server.captured().len(), 10);
    eprintln!(
        "local TLS fixture: four files/two hosts previous two-transfer cap={previous_cap:?}; cold={cold:?}; verified CAS cache={cached_time:?}; 503+retry={retry:?}; max observed active={}",
        live.active_downloads
    );
}

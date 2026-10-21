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
        let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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
        let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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
        let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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
            let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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
        let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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
        let manager = DownloadManager::new(temp.downloader(server.broker(config())));
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

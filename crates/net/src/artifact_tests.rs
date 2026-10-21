// SPDX-License-Identifier: MIT
//! Real libcurl + existing synthetic TLS server, actual temporary files.
use super::*;
use crate::{
    ArtifactFile, ArtifactTransferRequest, StrongEtag, TransferCancellation, TransferError,
    TransferMode, TransferOutcome,
};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::AtomicBool;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
#[derive(Default)]
struct Observed {
    writes: AtomicUsize,
    truncates: AtomicUsize,
    dropped: AtomicBool,
}
struct TestFile {
    file: std::fs::File,
    observed: Arc<Observed>,
    gate: Option<Arc<WriteGate>>,
}
#[derive(Default)]
struct WriteGate {
    entered: AtomicBool,
    released: Mutex<bool>,
    changed: std::sync::Condvar,
}
struct ReleaseGate(Arc<WriteGate>);
impl Drop for ReleaseGate {
    fn drop(&mut self) {
        *self.0.released.lock().unwrap() = true;
        self.0.changed.notify_all();
    }
}
impl Drop for TestFile {
    fn drop(&mut self) {
        self.observed.dropped.store(true, Ordering::Release);
    }
}
impl Read for TestFile {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(out)
    }
}
impl Seek for TestFile {
    fn seek(&mut self, at: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(at)
    }
}
impl Write for TestFile {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.observed.writes.fetch_add(1, Ordering::Relaxed);
        if let Some(gate) = self.gate.take() {
            let mut released = gate.released.lock().unwrap();
            gate.entered.store(true, Ordering::Release);
            while !*released {
                released = gate.changed.wait(released).unwrap();
            }
        }
        self.file.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl ArtifactFile for TestFile {
    fn file_len(&self) -> std::io::Result<u64> {
        self.file.metadata().map(|m| m.len())
    }
    fn truncate(&mut self, size: u64) -> std::io::Result<()> {
        self.observed.truncates.fetch_add(1, Ordering::Relaxed);
        self.file.set_len(size)
    }
}
fn file(prefix: &[u8]) -> (Temp, TestFile, Arc<Observed>) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    for _ in 0..64 {
        let path = std::env::temp_dir().join(format!(
            "cobble-curl-{}-{}.part",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let Ok(mut file) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        else {
            continue;
        };
        file.write_all(prefix).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let observed = Arc::new(Observed::default());
        return (
            Temp(path),
            TestFile {
                file,
                observed: observed.clone(),
                gate: None,
            },
            observed,
        );
    }
    panic!("exclusive test file allocation failed");
}
fn request(mode: TransferMode, size: u64) -> ArtifactTransferRequest {
    ArtifactTransferRequest {
        url: "https://one.fixture.invalid/file".into(),
        credential: None,
        expected_size: size,
        max_bytes: size,
        mode,
    }
}
fn resume(offset: u64) -> TransferMode {
    TransferMode::Resume {
        offset,
        etag: StrongEtag::new("\"v1\"").unwrap(),
    }
}
async fn settled(observed: &Observed) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !observed.dropped.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("owned worker must release file");
}

#[tokio::test]
async fn curl_net004_real_tls_206_append_and_200_reset() {
    for (response, mode, prefix, expected_status) in [
        (
            "HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"\r\nConnection: close\r\n\r\ndef",
            resume(3),
            b"abc".as_slice(),
            206,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nETag: \"v2\"\r\nConnection: close\r\n\r\nabcdef",
            resume(3),
            b"old".as_slice(),
            200,
        ),
        (
            "HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef",
            TransferMode::Full,
            b"".as_slice(),
            200,
        ),
    ] {
        let server = Server::new(move |_| raw(response.as_bytes().to_vec())).await;
        let (temp, output, observed) = file(prefix);
        let cancel = TransferCancellation::default();
        let result = server
            .broker(config())
            .artifact_transfer(request(mode, 6), output, cancel.clone())
            .await;
        assert!(result.is_ok(), "{:?}", result.err());
        let receipt = result.unwrap();
        assert_eq!(receipt.outcome, TransferOutcome::Transferred);
        assert_eq!(receipt.metadata.status, expected_status);
        assert_eq!(std::fs::read(&temp.0).unwrap(), b"abcdef");
        assert!(
            !cancel.is_cancelled(),
            "successful attempt must not poison manager token"
        );
        assert_eq!(
            observed.truncates.load(Ordering::Relaxed),
            usize::from(expected_status == 200)
        );
        let captured = server.captured();
        if expected_status == 206 || !prefix.is_empty() {
            assert!(captured[0].to_ascii_lowercase().contains("range: bytes=3-"));
            assert!(
                captured[0]
                    .to_ascii_lowercase()
                    .contains("if-range: \"v1\"")
            );
        }
    }
    let version = curl::Version::get();
    eprintln!(
        "libcurl={} TLS={}",
        version.version(),
        version.ssl_version().unwrap_or("none")
    );
}

#[tokio::test]
async fn curl_net004_416_requires_whole_file_verification_without_writing() {
    for (prefix, total, complete) in [
        (b"abcdef".as_slice(), 6, true),
        (b"abcdef".as_slice(), 7, false),
        (b"abc".as_slice(), 6, false),
    ] {
        let server = Server::new(move |_| raw(format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Length: 4\r\nContent-Range: bytes */{total}\r\nConnection: close\r\n\r\noops").into_bytes())).await;
        let (temp, output, observed) = file(prefix);
        let result = server
            .broker(config())
            .artifact_transfer(
                request(resume(prefix.len() as u64), 6),
                output,
                TransferCancellation::default(),
            )
            .await;
        if complete {
            let receipt = result.unwrap();
            assert_eq!(receipt.outcome, TransferOutcome::NeedsWholeFileVerification);
            assert_eq!(receipt.metadata.body_bytes, 0);
        } else {
            assert!(matches!(result, Err(TransferError::InvalidRange)));
        }
        assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
        assert_eq!(observed.truncates.load(Ordering::Relaxed), 0);
        assert_eq!(std::fs::read(&temp.0).unwrap(), prefix);
    }
}

#[tokio::test]
async fn curl_net005_invalid_headers_never_touch_partial() {
    for (headers, wanted) in [
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v2\"",
            TransferError::RestartRequired,
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 2-4/6\r\nETag: \"v1\"",
            TransferError::InvalidRange,
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/7\r\nETag: \"v1\"",
            TransferError::InvalidRange,
        ),
        (
            "Content-Length: 4\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"",
            TransferError::SizeMismatch,
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"\r\nContent-Encoding: gzip",
            TransferError::Network(NetError::UnsupportedEncoding),
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"\r\nLast-Modified: one\r\nLast-Modified: two",
            TransferError::Network(NetError::Protocol),
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"\r\nContent-Type: MuLtIpArT/ByTeRaNgEs; boundary=synthetic",
            TransferError::InvalidRange,
        ),
        (
            "Content-Length: 3\r\nContent-Range: bytes 3-5/6\r\nETag: \"v1\"\r\nContent-Type: application/octet-stream\r\nContent-Type: application/octet-stream",
            TransferError::Network(NetError::Protocol),
        ),
    ] {
        let response =
            format!("HTTP/1.1 206 Partial Content\r\n{headers}\r\nConnection: close\r\n\r\ndef!");
        let server = Server::new(move |_| raw(response.clone().into_bytes())).await;
        let (temp, output, observed) = file(b"abc");
        let cancel = TransferCancellation::default();
        let result = server
            .broker(config())
            .artifact_transfer(request(resume(3), 6), output, cancel.clone())
            .await;
        assert!(
            matches!(result, Err(error) if error == wanted),
            "wrong header-gate result"
        );
        assert!(
            !cancel.is_cancelled(),
            "completed failed attempt must not poison manager token"
        );
        assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
        assert_eq!(observed.truncates.load(Ordering::Relaxed), 0);
        assert_eq!(std::fs::read(&temp.0).unwrap(), b"abc");
    }
}

#[tokio::test]
async fn curl_resume_redirect_requires_fresh_authorized_full_request() {
    for location in ["/other", "https://two.fixture.invalid/other"] {
        let server = Server::new(move |_| raw(format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 4\r\nConnection: close\r\n\r\nevil").into_bytes())).await;
        let (temp, output, observed) = file(b"abc");
        let result = server
            .broker(config())
            .artifact_transfer(
                request(resume(3), 6),
                output,
                TransferCancellation::default(),
            )
            .await;
        assert!(matches!(result, Err(TransferError::RestartRequired)));
        assert_eq!(server.captured().len(), 1);
        assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
        assert_eq!(std::fs::read(&temp.0).unwrap(), b"abc");
    }
}

#[tokio::test]
async fn curl_full_redirect_strips_credentials_irreversibly_and_revalidates_dns() {
    let server = Server::new(|input| {
        if input.starts_with("GET /file ") {
            raw(b"HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/second\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())
        } else if input.starts_with("GET /second ") {
            raw(b"HTTP/1.1 302 Found\r\nLocation: https://one.fixture.invalid/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())
        } else { ok() }
    }).await;
    let broker = server.broker(config());
    let credential = broker
        .register_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/").unwrap(),
            secret(),
        )
        .unwrap();
    let mut input = request(TransferMode::Full, 3);
    input.credential = Some(credential);
    let (_temp, output, _) = file(b"");
    broker
        .artifact_transfer(input, output, TransferCancellation::default())
        .await
        .unwrap();
    let captured = server.captured();
    assert_eq!(captured.len(), 3);
    assert!(has_secrets(&captured[0]));
    assert!(no_secrets(&captured[1]));
    assert!(no_secrets(&captured[2]));

    let server = Server::new(|_| raw(b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())).await;
    let routing = FixtureRouting::new(server.address);
    routing.routes.lock().unwrap().insert(
        "one.fixture.invalid".into(),
        VecDeque::from([vec![server.address], vec!["127.0.0.2:443".parse().unwrap()]]),
    );
    let broker = Broker::fixture(config(), routing, Some(root()));
    let (_temp, output, observed) = file(b"");
    let result = broker
        .artifact_transfer(
            request(TransferMode::Full, 3),
            output,
            TransferCancellation::default(),
        )
        .await;
    assert!(matches!(
        result,
        Err(TransferError::Network(NetError::ForbiddenDestination))
    ));
    assert_eq!(server.captured().len(), 1);
    assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn curl_tls_hostname_and_ca_verification_are_real() {
    let server = Server::new(|_| ok()).await;
    for trusted in [true, false] {
        let broker = Broker::fixture(
            config(),
            FixtureRouting::new(server.address),
            trusted.then(root),
        );
        let mut input = request(TransferMode::Full, 3);
        if trusted {
            input.url = "https://wrong.fixture.invalid/file".into();
        }
        let (_temp, output, observed) = file(b"");
        assert!(
            broker
                .artifact_transfer(input, output, TransferCancellation::default())
                .await
                .is_err()
        );
        assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
        assert_eq!(observed.truncates.load(Ordering::Relaxed), 0);
    }
    assert!(server.captured().is_empty());
}

#[tokio::test]
async fn curl_chunked_trailers_cannot_reenter_the_header_gate_or_truncate_body() {
    for (tail, success) in [
        ("0\r\n\r\n", true),
        ("0\r\nETag: \"late\"\r\n\r\n", false),
        ("0\r\nHTTP/1.1 200 OK\r\n\r\n", false),
    ] {
        let response = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n3\r\nabc\r\n{tail}"
        );
        let server = Server::new(move |_| raw(response.clone().into_bytes())).await;
        let (temp, output, observed) = file(b"");
        let result = server
            .broker(config())
            .artifact_transfer(
                request(TransferMode::Full, 3),
                output,
                TransferCancellation::default(),
            )
            .await;
        assert_eq!(result.is_ok(), success);
        assert_eq!(observed.truncates.load(Ordering::Relaxed), 1);
        assert_eq!(std::fs::read(&temp.0).unwrap(), b"abc");
    }
    // No Content-Length: actual decoded HTTP body bytes must still fit the
    // expected size. A short body fails completion; an extra chunk cannot
    // extend the controlled file beyond that size.
    for (chunks, wanted) in [
        ("2\r\nab\r\n0\r\n\r\n", TransferError::SizeMismatch),
        (
            "3\r\nabc\r\n1\r\nd\r\n0\r\n\r\n",
            TransferError::Network(NetError::BodyLimit),
        ),
    ] {
        let response = format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{chunks}"
        );
        let server = Server::new(move |_| raw(response.clone().into_bytes())).await;
        let (temp, output, observed) = file(b"");
        let result = server
            .broker(config())
            .artifact_transfer(
                request(TransferMode::Full, 3),
                output,
                TransferCancellation::default(),
            )
            .await;
        assert!(matches!(result, Err(error) if error == wanted));
        assert!(std::fs::metadata(&temp.0).unwrap().len() <= 3);
        assert_eq!(observed.truncates.load(Ordering::Relaxed), 1);
    }
}

#[tokio::test]
async fn curl_cancellation_deadline_and_queue_are_bounded() {
    let server = Server::new(|_| {
        vec![
            (
                Duration::ZERO,
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\na".to_vec(),
            ),
            (Duration::from_secs(10), b"bc".to_vec()),
        ]
    })
    .await;
    let mut settings = config();
    settings.max_concurrent = 1;
    settings.per_origin = 1;
    settings.no_progress_timeout = Duration::from_secs(2);
    settings.total_timeout = Duration::from_millis(500);
    let broker = server.broker(settings);
    let (_temp, mut output, observed) = file(b"");
    let gate = Arc::new(WriteGate::default());
    let release = ReleaseGate(gate.clone());
    output.gate = Some(gate.clone());
    let token = TransferCancellation::default();
    let child_broker = broker.clone();
    let child_token = token.clone();
    let first = tokio::spawn(async move {
        child_broker
            .artifact_transfer(request(TransferMode::Full, 3), output, child_token)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !gate.entered.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    token.cancel();
    assert!(matches!(
        first.await.unwrap(),
        Err(TransferError::Cancelled)
    ));
    // The async caller returned, but its live blocking worker still owns all
    // leases. No queued request can open a second connection during that gap.
    assert!(!observed.dropped.load(Ordering::Acquire));
    let (_temp2, second, _) = file(b"");
    let second_token = TransferCancellation::default();
    let child_broker = broker.clone();
    let child_token = second_token.clone();
    let queued = tokio::spawn(async move {
        child_broker
            .artifact_transfer(request(TransferMode::Full, 3), second, child_token)
            .await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    second_token.cancel();
    assert!(matches!(
        queued.await.unwrap(),
        Err(TransferError::Cancelled)
    ));
    assert_eq!(server.captured().len(), 1);

    // Queue time is part of the total deadline even while the original
    // worker is deterministically stuck inside a filesystem write callback.
    let (_temp3, third, _) = file(b"");
    let result = broker
        .artifact_transfer(
            request(TransferMode::Full, 3),
            third,
            TransferCancellation::default(),
        )
        .await;
    assert!(matches!(
        result,
        Err(TransferError::Network(NetError::Timeout(
            TimeoutPhase::Total
        )))
    ));
    assert!(!observed.dropped.load(Ordering::Acquire));
    assert_eq!(server.captured().len(), 1);
    drop(release);
    settled(&observed).await;

    let mut settings = config();
    settings.total_timeout = Duration::from_millis(200);
    let (_temp, output, observed) = file(b"");
    let token = TransferCancellation::default();
    let result = server
        .broker(settings)
        .artifact_transfer(request(TransferMode::Full, 3), output, token.clone())
        .await;
    assert!(matches!(
        result,
        Err(TransferError::Network(NetError::Timeout(_)))
    ));
    settled(&observed).await;
}

#[tokio::test]
async fn curl_dropping_future_keeps_worker_leases_until_actual_exit() {
    let server = Server::new(|_| ok()).await;
    let mut settings = config();
    settings.max_concurrent = 1;
    let broker = server.broker(settings);
    let (_temp, mut output, observed) = file(b"");
    let gate = Arc::new(WriteGate::default());
    let release = ReleaseGate(gate.clone());
    output.gate = Some(gate.clone());
    let token = TransferCancellation::default();
    let child_token = token.clone();
    let child = tokio::spawn(async move {
        broker
            .artifact_transfer(request(TransferMode::Full, 3), output, child_token)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !gate.entered.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    child.abort();
    assert!(matches!(child.await, Err(error) if error.is_cancelled()));
    assert!(token.is_cancelled());
    assert!(!observed.dropped.load(Ordering::Acquire));
    drop(release);
    settled(&observed).await;
}

struct SlowPreparation {
    file: TestFile,
    during_headers: bool,
}
impl Read for SlowPreparation {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(out)
    }
}
impl Write for SlowPreparation {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.file.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}
impl Seek for SlowPreparation {
    fn seek(&mut self, at: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(at)
    }
}
impl ArtifactFile for SlowPreparation {
    fn file_len(&self) -> std::io::Result<u64> {
        if !self.during_headers {
            std::thread::sleep(Duration::from_millis(650));
        }
        self.file.file_len()
    }
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        self.file.truncate(len)
    }
    fn accepted_headers(&mut self, _: crate::AcceptedArtifactHeaders) -> std::io::Result<()> {
        if self.during_headers {
            std::thread::sleep(Duration::from_millis(650));
        }
        Ok(())
    }
}
#[tokio::test]
async fn curl_no_progress_clock_does_not_charge_worker_or_accepted_header_preparation_as_idle_body()
{
    for during_headers in [false, true] {
        let server = Server::new(|_| ok()).await;
        let (_temp, file, _) = file(b"");
        let result = server
            .broker(config())
            .artifact_transfer(
                request(TransferMode::Full, 3),
                SlowPreparation {
                    file,
                    during_headers,
                },
                TransferCancellation::default(),
            )
            .await;
        assert!(
            result.is_ok(),
            "preparation was incorrectly charged as idle body: {:?}",
            result.err()
        );
        assert_eq!(server.captured().len(), 1);
    }
    // Preparation is still covered by the total deadline. Its blocking worker
    // retains the file/leases until it actually notices expiration and exits.
    for during_headers in [false, true] {
        let server = Server::new(|_| {
            raw(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec())
        })
        .await;
        let (_temp, file, observed) = file(b"");
        let mut settings = config();
        settings.total_timeout = Duration::from_millis(200);
        let result = server
            .broker(settings)
            .artifact_transfer(
                request(TransferMode::Full, 0),
                SlowPreparation {
                    file,
                    during_headers,
                },
                TransferCancellation::default(),
            )
            .await;
        assert!(matches!(
            result,
            Err(TransferError::Network(NetError::Timeout(
                TimeoutPhase::Total
            )))
        ));
        settled(&observed).await;
        assert_eq!(server.captured().len(), usize::from(during_headers));
        assert_eq!(observed.writes.load(Ordering::Relaxed), 0);
    }
}

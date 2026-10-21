// SPDX-License-Identifier: MIT
//! Original TLS fixture generated once with OpenSSL 3.6.5 on 2026-10-21:
//! RSA-2048 root + RSA-2048 server CSR, SHA-256 signing, serverAuth EKU,
//! SAN one.fixture.invalid/two.fixture.invalid, 3650 days, PKCS12 empty password.
//! This public, synthetic test key is not a user/provider/release credential.
//! Leaf expires 2036-10-18; regenerate before expiry. Test trust/loopback routing
//! are compiled only under cfg(test), never enabled by a production feature.

use crate::policy::{Endpoint, public_ip, validate_addresses};
use crate::{Broker, BrokerConfig, CredentialScope, NetError, ProviderCredentials, TimeoutPhase};
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

#[derive(Clone)]
pub(crate) struct FixtureRouting {
    routes: Arc<Mutex<HashMap<String, VecDeque<Vec<SocketAddr>>>>>,
    allow_loopback: bool,
    loopback_address: SocketAddr,
    delay: Duration,
    calls: Arc<AtomicUsize>,
}
impl FixtureRouting {
    fn new(address: SocketAddr) -> Self {
        let mut routes = HashMap::new();
        for host in [
            "one.fixture.invalid",
            "two.fixture.invalid",
            "wrong.fixture.invalid",
        ] {
            routes.insert(host.to_owned(), VecDeque::from([vec![address]]));
        }
        Self {
            routes: Arc::new(Mutex::new(routes)),
            allow_loopback: true,
            loopback_address: address,
            delay: Duration::ZERO,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub(crate) async fn resolve(&self, host: &str) -> Result<Vec<SocketAddr>, NetError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(self.delay).await;
        let addresses = {
            let mut routes = self.routes.lock().unwrap();
            let entries = routes.get_mut(host).ok_or(NetError::Resolution)?;
            if entries.len() > 1 {
                entries.pop_front().unwrap()
            } else {
                entries.front().ok_or(NetError::Resolution)?.clone()
            }
        };
        if self.allow_loopback && addresses.len() == 1 && addresses[0] == self.loopback_address {
            Ok(addresses)
        } else {
            validate_addresses(addresses)
        }
    }
}

type Plan = Vec<(Duration, Vec<u8>)>;
struct Server {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Server {
    async fn new(handler: impl Fn(&str) -> Plan + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let identity = native_tls::Identity::from_pkcs12(
            include_bytes!("../tests/fixtures/synthetic-identity.p12"),
            "",
        )
        .unwrap();
        let acceptor =
            tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let handler = Arc::new(handler);
        let task = tokio::spawn(async move {
            let mut children = tokio::task::JoinSet::new();
            loop {
                let Ok((tcp, _)) = listener.accept().await else {
                    break;
                };
                let acceptor = acceptor.clone();
                let handler = handler.clone();
                let captured = captured.clone();
                children.spawn(async move {
                    let Ok(mut stream) = acceptor.accept(tcp).await else {
                        return;
                    };
                    let mut input = Vec::new();
                    while !input.ends_with(b"\r\n\r\n") {
                        let mut byte = [0];
                        match stream.read(&mut byte).await {
                            Ok(1) => input.push(byte[0]),
                            _ => return,
                        }
                        if input.len() > 16 * 1024 {
                            return;
                        }
                    }
                    let input = String::from_utf8(input).unwrap();
                    captured.lock().unwrap().push(input.clone());
                    for (delay, bytes) in handler(&input) {
                        tokio::time::sleep(delay).await;
                        if stream.write_all(&bytes).await.is_err() {
                            return;
                        }
                    }
                    let _ = stream.shutdown().await;
                });
                while children.try_join_next().is_some() {}
            }
        });
        Self {
            address,
            requests,
            task,
        }
    }
    fn broker(&self, config: BrokerConfig) -> Broker {
        Broker::fixture(config, FixtureRouting::new(self.address), Some(root()))
    }
    fn captured(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn root() -> native_tls::Certificate {
    native_tls::Certificate::from_der(include_bytes!("../tests/fixtures/synthetic-root.der"))
        .unwrap()
}
fn config() -> BrokerConfig {
    BrokerConfig {
        connect_timeout: Duration::from_secs(2),
        no_progress_timeout: Duration::from_millis(500),
        total_timeout: Duration::from_secs(4),
        ..BrokerConfig::default()
    }
}
fn raw(bytes: impl Into<Vec<u8>>) -> Plan {
    vec![(Duration::ZERO, bytes.into())]
}
fn ok() -> Plan {
    raw(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc".to_vec())
}
fn secret() -> ProviderCredentials {
    ProviderCredentials {
        bearer: Some("synthetic-bearer".into()),
        api_key: Some("synthetic-key".into()),
        cookie: Some("synthetic-cookie=yes".into()),
    }
}
fn has_secrets(request: &str) -> bool {
    request.contains("synthetic-bearer")
        && request.contains("synthetic-key")
        && request.contains("synthetic-cookie=yes")
}
fn no_secrets(request: &str) -> bool {
    !request.contains("synthetic-bearer")
        && !request.contains("synthetic-key")
        && !request.contains("synthetic-cookie")
}

#[test]
fn net001_public_address_and_url_policy() {
    for ip in [
        "0.1.2.3",
        "10.1.2.3",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.254",
        "172.16.1.1",
        "192.0.0.9",
        "192.0.2.1",
        "192.31.196.1",
        "192.52.193.1",
        "192.88.99.1",
        "192.168.1.1",
        "192.175.48.1",
        "198.18.1.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "64:ff9b:1::a00:1",
        "100::1",
        "2001::1",
        "2001:db8::1",
        "2002:808:808::1",
        "2620:4f:8000::1",
        "3fff::1",
        "fc00::1",
        "fe80::1",
        "ff02::1",
        "2001:4860::5efe:c0a8:1",
    ] {
        assert!(!public_ip(ip.parse().unwrap()), "{ip}");
    }
    for ip in [
        "8.8.8.8",
        "1.1.1.1",
        "172.32.0.1",
        "2001:4860:4860::8888",
        "2606:4700:4700::1111",
    ] {
        assert!(public_ip(ip.parse().unwrap()), "{ip}");
    }
    for url in [
        "http://example.com/",
        "file:///etc/passwd",
        "https://name:secret@example.com/",
        "https://@example.com/",
        "https://example.com:444/",
        "https://localhost/",
        "https://x.local/",
        "https://127.1/",
        "https://2130706433/",
        "https://0x08080808/",
        "https://example.com./",
        "https://example.com/#secret",
        "https://example.com/\\bad",
    ] {
        assert!(Endpoint::parse(url).is_err(), "{url}");
    }
    let endpoint = Endpoint::parse("https://one.fixture.invalid/v1/start").unwrap();
    for location in ["https://0x08080808/", "//0x08080808/", "https://127.0.0.1/"] {
        assert!(endpoint.redirect(location).is_err());
    }
    assert!(
        validate_addresses(vec![
            "8.8.8.8:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap()
        ])
        .is_err()
    );
    assert!(validate_addresses(vec!["8.8.8.8:443".parse().unwrap(); 33]).is_err());
}

#[tokio::test]
async fn net001_pinned_socket_real_tls_and_certificate_verification() {
    let server = Server::new(|_| ok()).await;
    let routing = FixtureRouting::new(server.address);
    let broker = Broker::fixture(config(), routing.clone(), Some(root()));
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/v1?opaque=query", None, 3)
            .await
            .unwrap()
            .body,
        b"abc"
    );
    assert_eq!(routing.calls.load(Ordering::Relaxed), 1);
    assert!(server.captured()[0].contains("host: one.fixture.invalid\r\n"));
    assert!(server.captured()[0].contains("accept-encoding: identity\r\n"));
    assert_eq!(
        broker
            .get("https://wrong.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::Tls)
    );
    let untrusted = Broker::fixture(config(), FixtureRouting::new(server.address), None);
    assert_eq!(
        untrusted
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::Tls)
    );
    assert_eq!(
        server.captured().len(),
        1,
        "TLS failures must not send HTTP credentials or data"
    );
}

#[tokio::test]
async fn net001_mixed_dns_redirect_private_and_resolution_deadline() {
    let server = Server::new(|_| raw(b"HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1/metadata\r\nContent-Length: 0\r\n\r\n".to_vec())).await;
    assert_eq!(
        server
            .broker(config())
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::ForbiddenDestination)
    );
    let mut routing = FixtureRouting::new(server.address);
    routing.allow_loopback = false;
    routing.routes.lock().unwrap().insert(
        "one.fixture.invalid".into(),
        VecDeque::from([vec![
            "8.8.8.8:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap(),
        ]]),
    );
    let broker = Broker::fixture(config(), routing, Some(root()));
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::ForbiddenDestination)
    );
    assert_eq!(server.captured().len(), 1);
    let mut routing = FixtureRouting::new(server.address);
    routing.delay = Duration::from_millis(150);
    let broker = Broker::fixture(
        BrokerConfig {
            total_timeout: Duration::from_millis(40),
            ..config()
        },
        routing,
        Some(root()),
    );
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::Timeout(TimeoutPhase::Total))
    );
}

#[tokio::test]
async fn net001_same_origin_redirect_revalidates_changed_dns_answers() {
    let server = Server::new(|_| {
        raw(b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\n\r\n".to_vec())
    })
    .await;
    let routing = FixtureRouting::new(server.address);
    routing.routes.lock().unwrap().insert(
        "one.fixture.invalid".into(),
        VecDeque::from([vec![server.address], vec!["127.0.0.1:443".parse().unwrap()]]),
    );
    let broker = Broker::fixture(config(), routing.clone(), Some(root()));
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::ForbiddenDestination)
    );
    assert_eq!(routing.calls.load(Ordering::Relaxed), 2);
    assert_eq!(
        server.captured().len(),
        1,
        "second DNS answer must be rejected before socket/HTTP use"
    );
}

#[tokio::test]
async fn net002_cross_origin_strips_all_credentials_irreversibly() {
    let server = Server::new(|request| {
        if request.starts_with("GET /v1/start ") { raw(b"HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/v1/hop\r\nContent-Length: 0\r\n\r\n".to_vec()) }
        else if request.starts_with("GET /v1/hop ") { raw(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: https://one.fixture.invalid/v1/final\r\nContent-Length: 0\r\n\r\n".to_vec()) } else { ok() }
    }).await;
    let broker = server.broker(config());
    let id = broker
        .register_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/v1").unwrap(),
            secret(),
        )
        .unwrap();
    let response = broker
        .get("https://one.fixture.invalid/v1/start", Some(id), 3)
        .await
        .unwrap();
    assert_eq!(response.metadata.redirects, 2);
    let requests = server.captured();
    assert_eq!(requests.len(), 3);
    assert!(has_secrets(&requests[0]));
    assert!(no_secrets(&requests[1]));
    assert!(no_secrets(&requests[2]));
}

#[tokio::test]
async fn net002_path_boundary_encoding_dot_segments_and_cross_broker_ids() {
    let server = Server::new(|_| ok()).await;
    let broker = server.broker(config());
    let id = broker
        .register_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/v1").unwrap(),
            secret(),
        )
        .unwrap();
    for path in [
        "/v1",
        "/v1/file.json",
        "/v1evil",
        "/v1/%2ffile",
        "/v1/%252efile",
        "/v1/../v1/file",
        "/v1/a;key",
    ] {
        broker
            .get(
                &format!("https://one.fixture.invalid{path}"),
                Some(id.clone()),
                3,
            )
            .await
            .unwrap();
    }
    let requests = server.captured();
    assert!(has_secrets(&requests[0]));
    assert!(has_secrets(&requests[1]));
    assert!(requests[2..].iter().all(|request| no_secrets(request)));
    let other = server.broker(config());
    let error = other
        .get(
            "https://one.fixture.invalid/?token=never-print-this",
            Some(id),
            3,
        )
        .await
        .err()
        .unwrap();
    assert_eq!(error, NetError::CredentialScope);
    assert!(!error.to_string().contains("never-print"));
    for prefix in ["/v1/%2f", "/v1/../secret", "/v1//x", "/v1?x"] {
        assert!(CredentialScope::new("https://one.fixture.invalid", prefix).is_err());
    }
}

#[tokio::test]
async fn net002_relative_redirect_dot_path_does_not_restore_credentials() {
    let server = Server::new(|request| {
        if request.starts_with("GET /v1/start ") {
            raw(
                b"HTTP/1.1 302 Found\r\nLocation: ../v1/final\r\nContent-Length: 0\r\n\r\n"
                    .to_vec(),
            )
        } else {
            ok()
        }
    })
    .await;
    let broker = server.broker(config());
    let id = broker
        .register_credentials(
            CredentialScope::new("https://one.fixture.invalid", "/v1").unwrap(),
            secret(),
        )
        .unwrap();
    broker
        .get("https://one.fixture.invalid/v1/start", Some(id), 3)
        .await
        .unwrap();
    let requests = server.captured();
    assert_eq!(requests.len(), 2);
    assert!(has_secrets(&requests[0]));
    assert!(no_secrets(&requests[1]));
}

#[tokio::test]
async fn net003_actual_chunked_body_encoding_headers_trailers_and_errors() {
    let replies = [
        (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nabcd\r\n4\r\nefgh\r\n0\r\n\r\n".to_vec(), NetError::BodyLimit),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n".to_vec(), NetError::BodyLimit),
        (b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 3\r\n\r\nabc".to_vec(), NetError::UnsupportedEncoding),
        (format!("HTTP/1.1 200 OK\r\nX-Huge: {}\r\nContent-Length: 0\r\n\r\n", "x".repeat(20_000)).into_bytes(), NetError::Protocol),
        (format!("HTTP/1.1 200 OK\r\n{}Content-Length: 0\r\n\r\n", "X-Item: x\r\n".repeat(65)).into_bytes(), NetError::Protocol),
        (b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nabc".to_vec(), NetError::Protocol),
        (format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\na\r\n0\r\nX-Huge: {}\r\n\r\n", "x".repeat(20_000)).into_bytes(), NetError::Protocol),
    ];
    for (reply, expected) in replies {
        let server = Server::new(move |_| raw(reply.clone())).await;
        let mut sink = Vec::new();
        assert_eq!(
            server
                .broker(config())
                .get_to("https://one.fixture.invalid/", None, 6, &mut sink)
                .await
                .err(),
            Some(expected)
        );
        assert!(sink.len() <= 6);
    }
    let server = Server::new(|_| {
        raw(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 7\r\n\r\nprivate".to_vec())
    })
    .await;
    let response = server
        .broker(config())
        .get("https://one.fixture.invalid/", None, 3)
        .await
        .unwrap();
    assert_eq!(response.metadata.status, 403);
    assert!(response.body.is_empty());
}

#[tokio::test]
async fn net003_body_headers_and_total_redirect_deadlines() {
    let server = Server::new(|_| {
        vec![
            (
                Duration::ZERO,
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n".to_vec(),
            ),
            (Duration::from_millis(200), b"abc".to_vec()),
        ]
    })
    .await;
    let broker = server.broker(BrokerConfig {
        no_progress_timeout: Duration::from_millis(50),
        ..config()
    });
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::Timeout(TimeoutPhase::Body))
    );
    let server = Server::new(|_| {
        vec![(
            Duration::from_millis(200),
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
        )]
    })
    .await;
    assert_eq!(
        server
            .broker(BrokerConfig {
                no_progress_timeout: Duration::from_millis(50),
                ..config()
            })
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .err(),
        Some(NetError::Timeout(TimeoutPhase::Headers))
    );
    let server = Server::new(|_| {
        raw(b"HTTP/1.1 302 Found\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n".to_vec())
    })
    .await;
    assert_eq!(
        server
            .broker(BrokerConfig {
                max_redirects: 2,
                ..config()
            })
            .get("https://one.fixture.invalid/loop", None, 3)
            .await
            .err(),
        Some(NetError::RedirectLimit)
    );
    assert_eq!(server.captured().len(), 3);
}

#[tokio::test]
async fn net003_continuous_small_chunks_still_hit_total_deadline() {
    let server = Server::new(|_| {
        let mut plan = raw(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n".to_vec());
        plan.extend((0..10).map(|_| (Duration::from_millis(30), b"x".to_vec())));
        plan
    })
    .await;
    let broker = server.broker(BrokerConfig {
        total_timeout: Duration::from_millis(140),
        no_progress_timeout: Duration::from_millis(80),
        ..config()
    });
    let mut sink = Vec::new();
    assert_eq!(
        broker
            .get_to("https://one.fixture.invalid/", None, 10, &mut sink)
            .await
            .err(),
        Some(NetError::Timeout(TimeoutPhase::Total))
    );
    assert!(!sink.is_empty());
    assert!(sink.len() < 10);
}

struct GateWriter {
    started: Option<oneshot::Sender<()>>,
    release: oneshot::Receiver<()>,
    ready: bool,
}
impl AsyncWrite for GateWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        if !self.ready {
            match Pin::new(&mut self.release).poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(_) => self.ready = true,
            }
        }
        Poll::Ready(Ok(bytes.len()))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
struct PendingWriter {
    flush_only: bool,
}
impl AsyncWrite for PendingWriter {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.flush_only {
            Poll::Ready(Ok(bytes.len()))
        } else {
            Poll::Pending
        }
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Pending
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn net003_slow_sink_retains_global_and_origin_permits() {
    for (global, per_origin, second_host) in
        [(2, 1, "one.fixture.invalid"), (1, 8, "two.fixture.invalid")]
    {
        let server = Server::new(|_| ok()).await;
        let broker = server.broker(BrokerConfig {
            max_concurrent: global,
            per_origin,
            ..config()
        });
        let (started, ready) = oneshot::channel();
        let (release, gate) = oneshot::channel();
        let first = broker.clone();
        let task = tokio::spawn(async move {
            first
                .get_to(
                    "https://one.fixture.invalid/",
                    None,
                    3,
                    &mut GateWriter {
                        started: Some(started),
                        release: gate,
                        ready: false,
                    },
                )
                .await
        });
        ready.await.unwrap();
        let second = broker.clone();
        let next = tokio::spawn(async move {
            second
                .get(&format!("https://{second_host}/"), None, 3)
                .await
        });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(
            server.captured().len(),
            1,
            "buffered body with blocked sink still owns permits"
        );
        release.send(()).unwrap();
        assert!(task.await.unwrap().is_ok());
        assert!(next.await.unwrap().is_ok());
    }
}

#[tokio::test]
async fn net003_pending_sink_timeouts_and_cancellation_release_connections() {
    let server = Server::new(|_| ok()).await;
    let broker = server.broker(BrokerConfig {
        max_concurrent: 1,
        per_origin: 1,
        no_progress_timeout: Duration::from_millis(50),
        ..config()
    });
    for flush_only in [false, true] {
        assert_eq!(
            broker
                .get_to(
                    "https://one.fixture.invalid/",
                    None,
                    3,
                    &mut PendingWriter { flush_only }
                )
                .await
                .err(),
            Some(NetError::Timeout(TimeoutPhase::Body))
        );
        assert!(
            broker
                .get("https://one.fixture.invalid/", None, 3)
                .await
                .is_ok()
        );
    }
    let (started, ready) = oneshot::channel();
    let (_release, gate) = oneshot::channel();
    let cloned = broker.clone();
    let request = tokio::spawn(async move {
        cloned
            .get_to(
                "https://one.fixture.invalid/",
                None,
                3,
                &mut GateWriter {
                    started: Some(started),
                    release: gate,
                    ready: false,
                },
            )
            .await
    });
    ready.await.unwrap();
    request.abort();
    assert!(matches!(request.await, Err(error) if error.is_cancelled()));
    assert!(
        broker
            .get("https://one.fixture.invalid/", None, 3)
            .await
            .is_ok()
    );
}

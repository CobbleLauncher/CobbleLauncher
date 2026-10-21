// SPDX-License-Identifier: MIT
use crate::credentials::CredentialEntry;
use crate::credentials::CredentialPartition;
use crate::policy::{Endpoint, Origin, validate_addresses};
use crate::{CredentialId, CredentialScope, ProviderCredentials};
use bytes::Bytes;
use http::header::{
    ACCEPT_ENCODING, CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, HOST, LOCATION, USER_AGENT,
};
use http::{HeaderMap, Request};
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;
use std::{
    collections::HashMap,
    fmt,
    net::SocketAddr,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::{OwnedSemaphorePermit, Semaphore},
    time::timeout,
};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 64;
const HARD_BODY_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const HARD_BUFFER_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeoutPhase {
    Total,
    ConnectTls,
    Headers,
    Body,
}
/// Sanitized reasons only. Underlying errors can contain queries or secrets
/// and are deliberately neither retained nor displayed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetError {
    InvalidConfig,
    InvalidUrl,
    ForbiddenDestination,
    Resolution,
    ResolutionLimit,
    Connect,
    Tls,
    PeerMismatch,
    Protocol,
    HeaderLimit,
    BodyLimit,
    UnsupportedEncoding,
    RedirectLimit,
    CredentialScope,
    Sink,
    Timeout(TimeoutPhase),
}
impl fmt::Display for NetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "network broker: {self:?}")
    }
}
impl std::error::Error for NetError {}

#[derive(Clone, Copy, Debug)]
pub struct BrokerConfig {
    pub max_concurrent: usize,
    pub per_origin: usize,
    pub max_redirects: u8,
    pub connect_timeout: Duration,
    pub no_progress_timeout: Duration,
    pub total_timeout: Duration,
}
impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            max_concurrent: 12,
            per_origin: 4,
            max_redirects: 5,
            connect_timeout: Duration::from_secs(5),
            no_progress_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(30),
        }
    }
}
impl BrokerConfig {
    fn validate(self) -> Result<Self, NetError> {
        if !(1..=32).contains(&self.max_concurrent)
            || !(1..=8).contains(&self.per_origin)
            || self.max_redirects > 5
            || self.connect_timeout.is_zero()
            || self.no_progress_timeout.is_zero()
            || self.total_timeout.is_zero()
            || self.connect_timeout > Duration::from_secs(60)
            || self.no_progress_timeout > Duration::from_secs(60)
            || self.total_timeout > Duration::from_secs(300)
        {
            return Err(NetError::InvalidConfig);
        }
        Ok(self)
    }
}
/// No Debug: opaque response validators may contain server-controlled secrets.
pub struct ResponseMetadata {
    pub status: u16,
    pub body_bytes: u64,
    pub redirects: u8,
    /// Origin only, without path/query/fragment/userinfo.
    pub final_origin: String,
    pub content_length: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_range: Option<String>,
}
pub struct BufferedResponse {
    pub metadata: ResponseMetadata,
    pub body: Vec<u8>,
}
struct Inner {
    config: BrokerConfig,
    requests: Arc<Semaphore>,
    connections: Arc<Semaphore>,
    dns: Arc<Semaphore>,
    origins: Mutex<HashMap<Origin, Weak<Semaphore>>>,
    credentials: Mutex<Vec<CredentialEntry>>,
    identity: Arc<()>,
    tls: native_tls::TlsConnector,
    #[cfg(test)]
    fixture: Option<crate::tests::FixtureRouting>,
    #[cfg(test)]
    artifact_test_trust: bool,
}
/// Share one cloned broker across Core adapters for collective budgets.
/// Separate instances have separate budgets and credential namespaces.
#[derive(Clone)]
pub struct Broker {
    inner: Arc<Inner>,
}
impl Broker {
    pub fn new(config: BrokerConfig) -> Result<Self, NetError> {
        let config = config.validate()?;
        let tls = native_tls::TlsConnector::builder()
            .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
            .build()
            .map_err(|_| NetError::Tls)?;
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                requests: Arc::new(Semaphore::new(config.max_concurrent)),
                connections: Arc::new(Semaphore::new(config.max_concurrent)),
                dns: Arc::new(Semaphore::new(config.max_concurrent)),
                origins: Mutex::new(HashMap::new()),
                credentials: Mutex::new(Vec::new()),
                identity: Arc::new(()),
                tls,
                #[cfg(test)]
                fixture: None,
                #[cfg(test)]
                artifact_test_trust: false,
            }),
        })
    }
    pub fn register_credentials(
        &self,
        scope: CredentialScope,
        secrets: ProviderCredentials,
    ) -> Result<CredentialId, NetError> {
        self.register_credential_entry(scope, secrets, None)
    }
    /// Download registration binds a stable nonsecret permission partition to
    /// this exact broker entry. Duplicate partitions are refused; credential
    /// rotation needs an explicit future auth transition, not accidental reuse.
    pub fn register_download_credentials(
        &self,
        scope: CredentialScope,
        secrets: ProviderCredentials,
        partition: CredentialPartition,
    ) -> Result<CredentialId, NetError> {
        self.register_credential_entry(scope, secrets, Some(partition))
    }
    fn register_credential_entry(
        &self,
        scope: CredentialScope,
        secrets: ProviderCredentials,
        partition: Option<CredentialPartition>,
    ) -> Result<CredentialId, NetError> {
        let mut entry = CredentialEntry::new(scope, secrets)?;
        entry.download_partition = partition;
        let mut entries = self
            .inner
            .credentials
            .lock()
            .map_err(|_| NetError::CredentialScope)?;
        if entries.len() >= 64 {
            return Err(NetError::CredentialScope);
        }
        if partition.is_some()
            && entries
                .iter()
                .any(|entry| entry.download_partition == partition)
        {
            return Err(NetError::CredentialScope);
        }
        let id = CredentialId {
            index: entries.len(),
            identity: self.inner.identity.clone(),
        };
        entries.push(entry);
        Ok(id)
    }
    /// Small metadata fetch. Error statuses return metadata and an empty body.
    pub async fn get(
        &self,
        url: &str,
        credential: Option<CredentialId>,
        max_bytes: u64,
    ) -> Result<BufferedResponse, NetError> {
        if max_bytes > HARD_BUFFER_BYTES {
            return Err(NetError::BodyLimit);
        }
        let mut body = Vec::new();
        let metadata = self.get_to(url, credential, max_bytes, &mut body).await?;
        Ok(BufferedResponse { metadata, body })
    }
    /// Streaming GET for T008. Only successful final response bytes reach the
    /// sink, never redirect/error bodies. A failed sink may contain a bounded
    /// prefix, to be discarded by its transaction owner. No CAS commit implied.
    pub async fn get_to<W: AsyncWrite + Unpin>(
        &self,
        url: &str,
        credential: Option<CredentialId>,
        max_bytes: u64,
        output: &mut W,
    ) -> Result<ResponseMetadata, NetError> {
        if max_bytes > HARD_BODY_BYTES {
            return Err(NetError::BodyLimit);
        }
        if credential
            .as_ref()
            .is_some_and(|id| !Arc::ptr_eq(&id.identity, &self.inner.identity))
        {
            return Err(NetError::CredentialScope);
        }
        let endpoint = Endpoint::parse(url)?;
        timeout(
            self.inner.config.total_timeout,
            self.get_inner(endpoint, credential, max_bytes, output),
        )
        .await
        .map_err(|_| NetError::Timeout(TimeoutPhase::Total))?
    }
    async fn get_inner<W: AsyncWrite + Unpin>(
        &self,
        mut endpoint: Endpoint,
        credential: Option<CredentialId>,
        max_bytes: u64,
        output: &mut W,
    ) -> Result<ResponseMetadata, NetError> {
        let _request = self
            .inner
            .requests
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        let mut stripped = false;
        for hop in 0..=self.inner.config.max_redirects {
            let (mut response, connection) = self
                .send(&endpoint, credential.as_ref().filter(|_| !stripped))
                .await?;
            check_headers(response.headers())?;
            let status = response.status();
            if response
                .headers()
                .get_all(CONTENT_ENCODING)
                .iter()
                .any(|value| value.as_bytes() != b"identity")
            {
                return Err(NetError::UnsupportedEncoding);
            }
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
                if hop == self.inner.config.max_redirects {
                    return Err(NetError::RedirectLimit);
                }
                let locations: Vec<_> = response.headers().get_all(LOCATION).iter().collect();
                if locations.len() != 1 {
                    return Err(NetError::Protocol);
                }
                let next =
                    endpoint.redirect(locations[0].to_str().map_err(|_| NetError::InvalidUrl)?)?;
                stripped |= next.origin() != endpoint.origin();
                drop(response);
                connection.close().await;
                endpoint = next;
                continue;
            }
            let content_length = content_length(response.headers())?;
            let mut metadata = ResponseMetadata {
                status: status.as_u16(),
                body_bytes: 0,
                redirects: hop,
                final_origin: endpoint.origin().0,
                content_length,
                etag: header_text(response.headers(), "etag")?,
                last_modified: header_text(response.headers(), "last-modified")?,
                content_range: header_text(response.headers(), "content-range")?,
            };
            if status.is_success() {
                if content_length.is_some_and(|size| size > max_bytes) {
                    return Err(NetError::BodyLimit);
                }
                let result = self.copy_body(response.body_mut(), output, max_bytes).await;
                drop(response);
                connection.close().await;
                metadata.body_bytes = result?;
            } else {
                drop(response);
                connection.close().await;
            }
            return Ok(metadata);
        }
        Err(NetError::RedirectLimit)
    }
    async fn send(
        &self,
        endpoint: &Endpoint,
        credential: Option<&CredentialId>,
    ) -> Result<(hyper::Response<Incoming>, ConnectionGuard), NetError> {
        let addresses = self.resolve(endpoint).await?;
        let origin = self.origin_semaphore(endpoint.origin())?;
        let origin_lease = origin
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        let connection_lease = self
            .inner
            .connections
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        let tls = timeout(
            self.inner.config.connect_timeout,
            self.connect(endpoint, &addresses),
        )
        .await
        .map_err(|_| NetError::Timeout(TimeoutPhase::ConnectTls))??;
        let mut builder = hyper::client::conn::http1::Builder::new();
        builder
            .max_headers(MAX_HEADERS)
            .max_header_size(MAX_HEADER_BYTES)
            .max_buf_size(MAX_HEADER_BYTES);
        let (mut sender, connection) = builder
            .handshake::<_, Empty<Bytes>>(TokioIo::new(tls))
            .await
            .map_err(|_| NetError::Protocol)?;
        let leases = Arc::new(ConnectionLeases {
            _global: connection_lease,
            _origin: origin_lease,
        });
        let driver_leases = leases.clone();
        let handle = tokio::spawn(async move {
            // Leases remain with the actual socket until driver cancellation
            // is observed, even if the caller is dropped before awaiting abort.
            let _leases = driver_leases;
            let _ = connection.await;
        });
        let guard = ConnectionGuard {
            task: Some(handle),
            _leases: leases,
        };
        let mut request = Request::builder()
            .method("GET")
            .uri(endpoint.request_target())
            .header(HOST, endpoint.host_header())
            .header(USER_AGENT, "CobbleLauncher/0.1")
            .header(ACCEPT_ENCODING, "identity")
            .header(CONNECTION, "close")
            .body(Empty::<Bytes>::new())
            .map_err(|_| NetError::Protocol)?;
        if let Some(id) = credential {
            let entries = self
                .inner
                .credentials
                .lock()
                .map_err(|_| NetError::CredentialScope)?;
            entries
                .get(id.index)
                .ok_or(NetError::CredentialScope)?
                .attach(endpoint, request.headers_mut());
        }
        let response = timeout(
            self.inner.config.no_progress_timeout,
            sender.send_request(request),
        )
        .await
        .map_err(|_| NetError::Timeout(TimeoutPhase::Headers))?
        .map_err(|_| NetError::Protocol)?;
        Ok((response, guard))
    }
    async fn connect(
        &self,
        endpoint: &Endpoint,
        addresses: &[SocketAddr],
    ) -> Result<tokio_native_tls::TlsStream<TcpStream>, NetError> {
        let mut connected = None;
        for address in addresses {
            // Some CDN handshakes need over a second; the outer connection
            // deadline still bounds all address attempts together.
            if let Ok(Ok(stream)) = timeout(
                Duration::from_secs(3).min(self.inner.config.connect_timeout),
                TcpStream::connect(address),
            )
            .await
            {
                if stream.peer_addr().map_err(|_| NetError::PeerMismatch)? != *address {
                    return Err(NetError::PeerMismatch);
                }
                connected = Some(stream);
                break;
            }
        }
        let stream = connected.ok_or(NetError::Connect)?;
        tokio_native_tls::TlsConnector::from(self.inner.tls.clone())
            .connect(&endpoint.host(), stream)
            .await
            .map_err(|_| NetError::Tls)
    }
    async fn resolve(&self, endpoint: &Endpoint) -> Result<Vec<SocketAddr>, NetError> {
        #[cfg(test)]
        if let Some(fixture) = &self.inner.fixture {
            return fixture.resolve(&endpoint.host()).await;
        }
        if let Some(ip) = endpoint.url.host().and_then(|host| match host {
            url::Host::Ipv4(ip) => Some(std::net::IpAddr::V4(ip)),
            url::Host::Ipv6(ip) => Some(std::net::IpAddr::V6(ip)),
            _ => None,
        }) {
            return validate_addresses(vec![SocketAddr::new(ip, 443)]);
        }
        let host = endpoint.host();
        // Detached DNS tasks retain their lease through getaddrinfo completion;
        // a caller deadline cannot create unlimited orphaned blocking lookups.
        self.run_dns(async move {
            let addresses = tokio::net::lookup_host((host.as_str(), 443))
                .await
                .map_err(|_| NetError::Resolution)?;
            let mut bounded = Vec::new();
            for address in addresses {
                if bounded.len() == 32 {
                    return Err(NetError::ResolutionLimit);
                }
                bounded.push(address);
            }
            Ok(bounded)
        })
        .await
    }

    async fn run_dns<F>(&self, resolution: F) -> Result<Vec<SocketAddr>, NetError>
    where
        F: std::future::Future<Output = Result<Vec<SocketAddr>, NetError>> + Send + 'static,
    {
        let lease = self
            .inner
            .dns
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        tokio::spawn(async move {
            let _lease = lease;
            validate_addresses(resolution.await?)
        })
        .await
        .map_err(|_| NetError::Resolution)?
    }
    fn origin_semaphore(&self, origin: Origin) -> Result<Arc<Semaphore>, NetError> {
        let mut origins = self
            .inner
            .origins
            .lock()
            .map_err(|_| NetError::InvalidConfig)?;
        origins.retain(|_, weak| weak.strong_count() > 0);
        if let Some(permit) = origins.get(&origin).and_then(Weak::upgrade) {
            return Ok(permit);
        }
        let permit = Arc::new(Semaphore::new(self.inner.config.per_origin));
        origins.insert(origin, Arc::downgrade(&permit));
        Ok(permit)
    }
    async fn copy_body<W: AsyncWrite + Unpin>(
        &self,
        body: &mut Incoming,
        output: &mut W,
        max_bytes: u64,
    ) -> Result<u64, NetError> {
        let mut size = 0_u64;
        loop {
            let frame = timeout(self.inner.config.no_progress_timeout, body.frame())
                .await
                .map_err(|_| NetError::Timeout(TimeoutPhase::Body))?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame.map_err(|_| NetError::Protocol)?;
            if let Some(bytes) = frame.data_ref() {
                size = size
                    .checked_add(bytes.len() as u64)
                    .ok_or(NetError::BodyLimit)?;
                if size > max_bytes {
                    return Err(NetError::BodyLimit);
                }
                timeout(
                    self.inner.config.no_progress_timeout,
                    output.write_all(bytes),
                )
                .await
                .map_err(|_| NetError::Timeout(TimeoutPhase::Body))?
                .map_err(|_| NetError::Sink)?;
            }
            if let Some(trailers) = frame.trailers_ref() {
                check_headers(trailers)?;
            }
        }
        timeout(self.inner.config.no_progress_timeout, output.flush())
            .await
            .map_err(|_| NetError::Timeout(TimeoutPhase::Body))?
            .map_err(|_| NetError::Sink)?;
        Ok(size)
    }
}
pub(crate) struct ConnectionLeases {
    _global: OwnedSemaphorePermit,
    _origin: OwnedSemaphorePermit,
}
// Shared policy, credential namespace and resource budgets for the libcurl
// artifact adapter. Metadata requests continue to use the existing transport.
impl Broker {
    pub(crate) fn check_download_credentials(
        &self,
        endpoint: &Endpoint,
        credential: Option<&CredentialId>,
        partition: Option<CredentialPartition>,
    ) -> Result<(), NetError> {
        match (credential, partition) {
            (None, None) => Ok(()),
            (Some(id), Some(partition)) if Arc::ptr_eq(&id.identity, &self.inner.identity) => {
                let entries = self
                    .inner
                    .credentials
                    .lock()
                    .map_err(|_| NetError::CredentialScope)?;
                let entry = entries.get(id.index).ok_or(NetError::CredentialScope)?;
                if entry.download_partition == Some(partition) && entry.scope.matches(endpoint) {
                    Ok(())
                } else {
                    Err(NetError::CredentialScope)
                }
            }
            _ => Err(NetError::CredentialScope),
        }
    }
    pub(crate) fn artifact_config(&self) -> BrokerConfig {
        self.inner.config
    }
    #[cfg(test)]
    pub(crate) fn artifact_fixture(&self) -> (bool, bool) {
        (self.inner.fixture.is_some(), self.inner.artifact_test_trust)
    }

    pub(crate) async fn artifact_request_lease(&self) -> Result<OwnedSemaphorePermit, NetError> {
        self.inner
            .requests
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)
    }

    pub(crate) async fn artifact_hop(
        &self,
        endpoint: &Endpoint,
        credential: Option<&CredentialId>,
    ) -> Result<(Vec<SocketAddr>, HeaderMap, ConnectionLeases), NetError> {
        let addresses = self.resolve(endpoint).await?;
        let mut headers = HeaderMap::new();
        if let Some(id) = credential {
            if !Arc::ptr_eq(&id.identity, &self.inner.identity) {
                return Err(NetError::CredentialScope);
            }
            self.inner
                .credentials
                .lock()
                .map_err(|_| NetError::CredentialScope)?
                .get(id.index)
                .ok_or(NetError::CredentialScope)?
                .attach(endpoint, &mut headers);
        }
        let origin = self
            .origin_semaphore(endpoint.origin())?
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        let global = self
            .inner
            .connections
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| NetError::InvalidConfig)?;
        Ok((
            addresses,
            headers,
            ConnectionLeases {
                _global: global,
                _origin: origin,
            },
        ))
    }
}
struct ConnectionGuard {
    task: Option<tokio::task::JoinHandle<()>>,
    _leases: Arc<ConnectionLeases>,
}
impl ConnectionGuard {
    async fn close(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let _ = task.await;
        }
    }
}
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
fn check_headers(headers: &HeaderMap) -> Result<(), NetError> {
    let mut bytes = 0;
    let mut count = 0;
    for (name, value) in headers {
        bytes += name.as_str().len() + value.as_bytes().len() + 4;
        count += 1;
    }
    if count > MAX_HEADERS || bytes > MAX_HEADER_BYTES {
        return Err(NetError::HeaderLimit);
    }
    Ok(())
}
fn header_text(headers: &HeaderMap, name: &str) -> Result<Option<String>, NetError> {
    let values: Vec<_> = headers.get_all(name).iter().collect();
    if values.len() > 1 {
        return Err(NetError::Protocol);
    }
    values
        .first()
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| NetError::Protocol)
        })
        .transpose()
}
fn content_length(headers: &HeaderMap) -> Result<Option<u64>, NetError> {
    header_text(headers, CONTENT_LENGTH.as_str())?
        .map(|value| value.parse().map_err(|_| NetError::Protocol))
        .transpose()
}
#[cfg(test)]
impl Broker {
    pub(crate) fn fixture(
        config: BrokerConfig,
        routing: crate::tests::FixtureRouting,
        trust: Option<native_tls::Certificate>,
    ) -> Self {
        let mut broker = Self::new(config).unwrap();
        let inner = Arc::get_mut(&mut broker.inner).unwrap();
        inner.artifact_test_trust = trust.is_some();
        if let Some(root) = trust {
            inner.tls = native_tls::TlsConnector::builder()
                .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
                .add_root_certificate(root)
                .build()
                .unwrap();
        }
        inner.fixture = Some(routing);
        broker
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn net003_configuration_and_body_hard_limits_cannot_be_disabled() {
        assert!(
            BrokerConfig {
                max_redirects: 5,
                ..BrokerConfig::default()
            }
            .validate()
            .is_ok()
        );
        for config in [
            BrokerConfig {
                max_redirects: 6,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                max_concurrent: 0,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                max_concurrent: 33,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                per_origin: 0,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                per_origin: 9,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                connect_timeout: Duration::ZERO,
                ..BrokerConfig::default()
            },
            BrokerConfig {
                total_timeout: Duration::from_secs(301),
                ..BrokerConfig::default()
            },
        ] {
            assert_eq!(config.validate().err(), Some(NetError::InvalidConfig));
        }
        let broker = Broker::new(BrokerConfig::default()).unwrap();
        assert_eq!(
            broker
                .get("https://one.fixture.invalid/", None, HARD_BUFFER_BYTES + 1)
                .await
                .err(),
            Some(NetError::BodyLimit)
        );
        let mut sink = Vec::new();
        assert_eq!(
            broker
                .get_to(
                    "https://one.fixture.invalid/",
                    None,
                    HARD_BODY_BYTES + 1,
                    &mut sink
                )
                .await
                .err(),
            Some(NetError::BodyLimit)
        );
        assert!(sink.is_empty());
    }

    #[tokio::test]
    async fn net003_request_queue_wait_is_inside_total_deadline() {
        let broker = Broker::new(BrokerConfig {
            max_concurrent: 1,
            total_timeout: Duration::from_millis(20),
            ..BrokerConfig::default()
        })
        .unwrap();
        let _held = broker.inner.requests.clone().acquire_owned().await.unwrap();
        assert_eq!(
            broker
                .get("https://one.fixture.invalid/", None, 1)
                .await
                .err(),
            Some(NetError::Timeout(TimeoutPhase::Total))
        );
    }

    #[tokio::test]
    async fn net001_timed_out_dns_keeps_lease_until_resolver_really_finishes() {
        let broker = Broker::new(BrokerConfig {
            max_concurrent: 1,
            ..BrokerConfig::default()
        })
        .unwrap();
        let (entered, started) = tokio::sync::oneshot::channel();
        let (release, pending) = tokio::sync::oneshot::channel();
        let cloned = broker.clone();
        let caller = tokio::spawn(async move {
            timeout(
                Duration::from_millis(30),
                cloned.run_dns(async move {
                    entered.send(()).unwrap();
                    pending.await.unwrap();
                    Ok(vec!["8.8.8.8:443".parse().unwrap()])
                }),
            )
            .await
        });
        started.await.unwrap();
        assert!(caller.await.unwrap().is_err());
        assert_eq!(broker.inner.dns.available_permits(), 0);
        assert!(
            timeout(
                Duration::from_millis(20),
                broker.run_dns(async { Ok(vec!["8.8.8.8:443".parse().unwrap()]) })
            )
            .await
            .is_err()
        );
        release.send(()).unwrap();
        timeout(Duration::from_secs(1), async {
            while broker.inner.dns.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(broker.inner.dns.available_permits(), 1);
    }
}

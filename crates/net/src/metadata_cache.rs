// SPDX-License-Identifier: MIT
//! Bounded public metadata only. Credentialed requests never enter this cache.
use crate::{Broker, BufferedResponse, NetError, ResponseMetadata};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy)]
pub struct MetadataCacheConfig {
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub fresh_for: Duration,
    pub max_stale: Duration,
}
impl Default for MetadataCacheConfig {
    fn default() -> Self {
        Self {
            memory_bytes: 32 * 1024 * 1024,
            disk_bytes: 128 * 1024 * 1024,
            fresh_for: Duration::from_secs(300),
            max_stale: Duration::from_secs(7 * 86400),
        }
    }
}
#[derive(Clone, Copy, Default, serde::Serialize)]
pub struct MetadataCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub refreshes: u64,
    pub memory_entries: u64,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
}
#[derive(Serialize, Deserialize)]
struct Stored {
    body: Vec<u8>,
    fetched: u64,
    origin: String,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
}
struct Entry {
    stored: Stored,
    refreshing: AtomicBool,
}
struct Inner {
    memory: Cache<String, Arc<Entry>>,
    root: PathBuf,
    config: MetadataCacheConfig,
    hits: AtomicU64,
    misses: AtomicU64,
    refreshes: AtomicU64,
    generation: AtomicU64,
    disk: tokio::sync::Mutex<()>,
}
#[derive(Clone)]
pub struct PublicMetadataCache(Arc<Inner>);
#[derive(Clone)]
enum FetchError {
    Net(NetError),
    Http(Box<BufferedResponse>),
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
impl PublicMetadataCache {
    pub fn new(root: PathBuf, config: MetadataCacheConfig) -> Self {
        Self(Arc::new(Inner {
            memory: Cache::builder()
                .max_capacity(config.memory_bytes)
                .weigher(|_: &String, value: &Arc<Entry>| {
                    value
                        .stored
                        .body
                        .len()
                        .saturating_add(256)
                        .min(u32::MAX as usize) as u32
                })
                .build(),
            root,
            config,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            refreshes: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            disk: tokio::sync::Mutex::new(()),
        }))
    }
    /// Uses the same OS cache root as the artifact store, in a separate directory.
    pub fn shared() -> Option<Self> {
        static CACHE: OnceLock<Option<PublicMetadataCache>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                dirs::cache_dir().map(|root| {
                    Self::new(
                        root.join("CobbleLauncher").join("public-metadata"),
                        MetadataCacheConfig::default(),
                    )
                })
            })
            .clone()
    }
    fn key(url: &str, source: &str, limit: u64) -> Result<String, NetError> {
        let parsed = url::Url::parse(url).map_err(|_| NetError::InvalidUrl)?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.fragment().is_some()
        {
            return Err(NetError::InvalidUrl);
        }
        // URL includes origin and all filters; source selection is a separate partition.
        Ok(format!(
            "{:x}",
            Sha256::digest(format!("{source}\0{limit}\0{url}").as_bytes())
        ))
    }
    pub async fn invalidate(&self, url: &str, source: &str, limit: u64) -> Result<(), NetError> {
        let key = Self::key(url, source, limit)?;
        self.0.generation.fetch_add(1, Ordering::SeqCst);
        self.0.memory.invalidate(&key).await;
        let _guard = self.0.disk.lock().await;
        let _ = std::fs::remove_file(self.0.root.join(key));
        Ok(())
    }
    pub async fn clear(&self) {
        self.0.generation.fetch_add(1, Ordering::SeqCst);
        self.0.memory.invalidate_all();
        let _guard = self.0.disk.lock().await;
        if let Ok(files) = std::fs::read_dir(&self.0.root) {
            for file in files.flatten() {
                let name = file.file_name();
                if name.to_str().is_some_and(|name| {
                    name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())
                }) {
                    let _ = std::fs::remove_file(file.path());
                }
            }
        }
        self.0.memory.run_pending_tasks().await;
    }
    pub async fn stats(&self) -> MetadataCacheStats {
        self.0.memory.run_pending_tasks().await;
        MetadataCacheStats {
            hits: self.0.hits.load(Ordering::Relaxed),
            misses: self.0.misses.load(Ordering::Relaxed),
            refreshes: self.0.refreshes.load(Ordering::Relaxed),
            memory_entries: self.0.memory.entry_count(),
            memory_bytes: self.0.memory.weighted_size(),
            disk_bytes: std::fs::read_dir(&self.0.root)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|entry| entry.metadata().ok())
                .filter(|metadata| metadata.is_file())
                .map(|metadata| metadata.len())
                .sum(),
        }
    }
    async fn fetch(
        &self,
        broker: &Broker,
        url: &str,
        limit: u64,
        previous: Option<&Entry>,
    ) -> Result<Arc<Entry>, FetchError> {
        let response = if let Some(previous) = previous {
            broker
                .get_revalidated(
                    url,
                    limit,
                    crate::broker::Revalidation {
                        etag: previous.stored.etag.clone(),
                        last_modified: previous.stored.last_modified.clone(),
                    },
                )
                .await
        } else {
            broker.get(url, None, limit).await
        }
        .map_err(FetchError::Net)?;
        if response.metadata.status == 304
            && let Some(previous) = previous
        {
            return Ok(Arc::new(Entry {
                stored: Stored {
                    body: previous.stored.body.clone(),
                    fetched: now(),
                    origin: previous.stored.origin.clone(),
                    etag: response
                        .metadata
                        .etag
                        .or_else(|| previous.stored.etag.clone()),
                    last_modified: response
                        .metadata
                        .last_modified
                        .or_else(|| previous.stored.last_modified.clone()),
                },
                refreshing: AtomicBool::new(false),
            }));
        }
        if response.metadata.status != 200 {
            return Err(FetchError::Http(Box::new(response)));
        }
        Ok(Arc::new(Entry {
            stored: Stored {
                body: response.body,
                fetched: now(),
                origin: response.metadata.final_origin,
                etag: response.metadata.etag,
                last_modified: response.metadata.last_modified,
            },
            refreshing: AtomicBool::new(false),
        }))
    }
    async fn persist(&self, key: &str, entry: &Entry, generation: u64) {
        let _guard = self.0.disk.lock().await;
        if generation != self.0.generation.load(Ordering::SeqCst) {
            return;
        }
        let Ok(bytes) = serde_json::to_vec(&entry.stored) else {
            return;
        };
        if bytes.len() as u64 > self.0.config.disk_bytes {
            return;
        }
        if std::fs::create_dir_all(&self.0.root).is_err() {
            return;
        }
        let mut files: Vec<_> = std::fs::read_dir(&self.0.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                if !name.to_str().is_some_and(|name| {
                    name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
                }) {
                    return None;
                }
                let metadata = entry.metadata().ok()?;
                if !metadata.is_file() {
                    return None;
                }
                Some((metadata.modified().ok(), metadata.len(), entry.path()))
            })
            .collect();
        files.sort_by_key(|file| file.0);
        let mut total: u64 = files.iter().map(|file| file.1).sum();
        for (_, size, path) in files {
            if total.saturating_add(bytes.len() as u64) <= self.0.config.disk_bytes {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        // A damaged/interrupted cache file is ignored on the next read.
        let _ = std::fs::write(self.0.root.join(key), bytes);
    }
    pub async fn get(
        &self,
        broker: &Broker,
        url: &str,
        source: &str,
        limit: u64,
    ) -> Result<BufferedResponse, NetError> {
        if limit > 8 * 1024 * 1024 {
            return broker.get(url, None, limit).await;
        }
        let key = Self::key(url, source, limit)?;
        let generation = self.0.generation.load(Ordering::SeqCst);
        let cached = self.0.memory.get(&key).await;
        let entry = if let Some(entry) = cached {
            self.0.hits.fetch_add(1, Ordering::Relaxed);
            entry
        } else {
            self.0.misses.fetch_add(1, Ordering::Relaxed);
            let result = self
                .0
                .memory
                .try_get_with(key.clone(), async {
                    let path = self.0.root.join(&key);
                    if let Ok(metadata) = std::fs::metadata(&path)
                        && metadata.len() <= limit.saturating_mul(6).saturating_add(1024)
                        && let Ok(bytes) = std::fs::read(path)
                        && let Ok(stored) = serde_json::from_slice::<Stored>(&bytes)
                        && stored.body.len() as u64 <= limit
                        && now().saturating_sub(stored.fetched) <= self.0.config.max_stale.as_secs()
                    {
                        return Ok(Arc::new(Entry {
                            stored,
                            refreshing: AtomicBool::new(false),
                        }));
                    }
                    let entry = self.fetch(broker, url, limit, None).await?;
                    self.persist(&key, &entry, generation).await;
                    Ok::<_, FetchError>(entry)
                })
                .await;
            match result {
                Ok(entry) => {
                    if generation != self.0.generation.load(Ordering::SeqCst) {
                        self.0.memory.invalidate(&key).await;
                    }
                    entry
                }
                Err(error) => {
                    return match &*error {
                        FetchError::Net(error) => Err(*error),
                        FetchError::Http(response) => Ok((**response).clone()),
                    };
                }
            }
        };
        let age = now().saturating_sub(entry.stored.fetched);
        if age > self.0.config.max_stale.as_secs() {
            self.0.memory.invalidate(&key).await;
            let result = self
                .0
                .memory
                .try_get_with(key.clone(), async {
                    let fresh = self.fetch(broker, url, limit, None).await?;
                    self.persist(&key, &fresh, generation).await;
                    Ok::<_, FetchError>(fresh)
                })
                .await;
            return match result {
                Ok(fresh) => {
                    if generation != self.0.generation.load(Ordering::SeqCst) {
                        self.0.memory.invalidate(&key).await;
                    }
                    Ok(response(&fresh))
                }
                Err(error) => match &*error {
                    FetchError::Net(error) => Err(*error),
                    FetchError::Http(response) => Ok((**response).clone()),
                },
            };
        }
        if age >= self.0.config.fresh_for.as_secs()
            && !entry.refreshing.swap(true, Ordering::SeqCst)
        {
            let cache = self.clone();
            let broker = broker.clone();
            let url = url.to_owned();
            let previous = entry.clone();
            tokio::spawn(async move {
                cache.0.refreshes.fetch_add(1, Ordering::Relaxed);
                if let Ok(fresh) = cache.fetch(&broker, &url, limit, Some(&previous)).await {
                    cache.persist(&key, &fresh, generation).await;
                    if generation == cache.0.generation.load(Ordering::SeqCst) {
                        cache.0.memory.insert(key, fresh).await;
                    }
                }
                previous.refreshing.store(false, Ordering::SeqCst);
            });
        }
        Ok(response(&entry))
    }
}
fn response(entry: &Entry) -> BufferedResponse {
    BufferedResponse {
        body: entry.stored.body.clone(),
        metadata: ResponseMetadata {
            status: 200,
            body_bytes: entry.stored.body.len() as u64,
            redirects: 0,
            final_origin: entry.stored.origin.clone(),
            content_length: Some(entry.stored.body.len() as u64),
            etag: None,
            last_modified: None,
            content_range: None,
        },
    }
}
impl Broker {
    /// Explicit public metadata route. The normal GET and all credentials bypass caching.
    pub async fn get_public_metadata(
        &self,
        url: &str,
        source: &str,
        limit: u64,
    ) -> Result<BufferedResponse, NetError> {
        match PublicMetadataCache::shared() {
            Some(cache) => cache.get(self, url, source, limit).await,
            None => self.get(url, None, limit).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Server, config};
    fn root() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "cobble-metadata-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[tokio::test]
    async fn metadata_memory_and_disk_budgets_are_enforced() {
        let server = Server::new(|_| {
            vec![(
                Duration::ZERO,
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
            )]
        })
        .await;
        let broker = server.broker(config());
        let root = root();
        let cache = PublicMetadataCache::new(
            root.clone(),
            MetadataCacheConfig {
                memory_bytes: 512,
                disk_bytes: 300,
                ..MetadataCacheConfig::default()
            },
        );
        for index in 0..8 {
            cache
                .get(
                    &broker,
                    &format!("https://one.fixture.invalid/metadata?index={index}"),
                    "official",
                    1024,
                )
                .await
                .unwrap();
        }
        let stats = cache.stats().await;
        assert!(stats.memory_bytes <= 512);
        assert!(stats.disk_bytes <= 300);
        cache.clear().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn metadata_conditionals_revalidate_without_body_and_strip_cross_origin() {
        let server = Server::new(|request| {
            let response = if request.starts_with("GET /redirect") {
                "HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            } else if request.to_ascii_lowercase().contains("if-none-match: \"one\"") {
                "HTTP/1.1 304 Not Modified\r\nETag: \"one\"\r\nConnection: close\r\n\r\n"
            } else { "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nETag: \"one\"\r\nConnection: close\r\n\r\n{}" };
            vec![(Duration::ZERO,response.as_bytes().to_vec())]
        }).await;
        let broker = server.broker(config());
        let root = root();
        let cache = PublicMetadataCache::new(
            root.clone(),
            MetadataCacheConfig {
                fresh_for: Duration::ZERO,
                ..MetadataCacheConfig::default()
            },
        );
        let url = "https://one.fixture.invalid/metadata";
        assert_eq!(
            cache
                .get(&broker, url, "official", 1024)
                .await
                .unwrap()
                .body,
            b"{}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(server.captured().iter().any(|request| {
            request
                .to_ascii_lowercase()
                .contains("if-none-match: \"one\"")
        }));
        let response = broker
            .get_revalidated(
                "https://one.fixture.invalid/redirect",
                1024,
                crate::broker::Revalidation {
                    etag: Some("\"one\"".into()),
                    last_modified: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(response.metadata.status, 200);
        let captured = server.captured();
        assert!(
            !captured
                .iter()
                .find(|request| request.starts_with("GET /final"))
                .unwrap()
                .to_ascii_lowercase()
                .contains("if-none-match")
        );
        cache.clear().await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn metadata_hit_coalescing_source_filters_and_disk_restart() {
        let server = Server::new(|_| {
            vec![(
                Duration::from_millis(30),
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
            )]
        })
        .await;
        let broker = server.broker(config());
        let root = root();
        let cache = PublicMetadataCache::new(root.clone(), MetadataCacheConfig::default());
        let url = "https://one.fixture.invalid/metadata?loader=fabric";
        let (one, two) = tokio::join!(
            cache.get(&broker, url, "official", 1024),
            cache.get(&broker, url, "official", 1024)
        );
        assert_eq!(one.unwrap().body, b"{}");
        assert_eq!(two.unwrap().body, b"{}");
        cache.get(&broker, url, "official", 1024).await.unwrap();
        assert_eq!(server.captured().len(), 1);
        cache.get(&broker, url, "mirror", 1024).await.unwrap();
        cache
            .get(
                &broker,
                "https://one.fixture.invalid/metadata?loader=forge",
                "official",
                1024,
            )
            .await
            .unwrap();
        assert_eq!(server.captured().len(), 3);
        let restarted = PublicMetadataCache::new(root.clone(), MetadataCacheConfig::default());
        restarted.get(&broker, url, "official", 1024).await.unwrap();
        assert_eq!(server.captured().len(), 3);
        restarted.invalidate(url, "official", 1024).await.unwrap();
        restarted.get(&broker, url, "official", 1024).await.unwrap();
        assert_eq!(server.captured().len(), 4);
        restarted.clear().await;
        assert_eq!(restarted.stats().await.memory_entries, 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn metadata_stale_returns_immediately_and_refreshes_once_with_offline_fallback() {
        let server = Server::new(|_| {
            vec![(
                Duration::from_millis(80),
                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
            )]
        })
        .await;
        let broker = server.broker(config());
        let root = root();
        let cache = PublicMetadataCache::new(
            root.clone(),
            MetadataCacheConfig {
                fresh_for: Duration::ZERO,
                ..MetadataCacheConfig::default()
            },
        );
        let url = "https://one.fixture.invalid/metadata";
        cache.get(&broker, url, "official", 1024).await.unwrap();
        let before = std::time::Instant::now();
        let (one, two) = tokio::join!(
            cache.get(&broker, url, "official", 1024),
            cache.get(&broker, url, "official", 1024)
        );
        assert_eq!(one.unwrap().body, b"{}");
        assert_eq!(two.unwrap().body, b"{}");
        assert!(before.elapsed() < Duration::from_millis(60));
        tokio::time::sleep(Duration::from_millis(180)).await;
        assert_eq!(server.captured().len(), 2);
        drop(server);
        assert_eq!(
            cache
                .get(&broker, url, "official", 1024)
                .await
                .unwrap()
                .body,
            b"{}"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(
            cache
                .get(&broker, url, "official", 1024)
                .await
                .unwrap()
                .body,
            b"{}"
        );
        cache.clear().await;
        let _ = std::fs::remove_dir_all(root);
    }
}

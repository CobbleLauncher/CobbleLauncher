// SPDX-License-Identifier: MIT
//! Public Mojang file URL mappings, independently implemented from
//! https://bmclapidoc.bangbang93.com/ (usage section). Never maps auth APIs.
use cobble_net::{Broker, NetError, TimeoutPhase};
use sha1::{Digest, Sha1};
use std::fmt;
use std::{
    future::Future,
    sync::Mutex,
    time::{Duration, Instant},
};
use url::Url;

const BMCLAPI: &str = "https://bmclapi2.bangbang93.com/";

#[derive(Clone, Debug, Default)]
pub enum DownloadSource {
    #[default]
    Automatic,
    Official,
    Bmclapi,
    /// Explicitly supplied BMCLAPI-compatible HTTPS base, not a provider claim.
    CustomBmclapi(Url),
}

const PREFERENCE_TTL: Duration = Duration::from_secs(300);
const METADATA_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Default)]
struct Preference {
    winner: Option<(bool, Instant)>,
    failed: Option<(bool, Instant)>,
}

static PREFERENCE: Mutex<Preference> = Mutex::new(Preference {
    winner: None,
    failed: None,
});

impl Preference {
    fn recent(value: Option<(bool, Instant)>) -> Option<bool> {
        value
            .filter(|(_, time)| time.elapsed() < PREFERENCE_TTL)
            .map(|(official, _)| official)
    }
    fn preferred(&self) -> bool {
        if let Some(failed) = Self::recent(self.failed) {
            !failed
        } else {
            Self::recent(self.winner).unwrap_or(true)
        }
    }
    fn cached(&self) -> bool {
        Self::recent(self.winner).is_some() || Self::recent(self.failed).is_some()
    }
}

#[derive(Debug)]
pub enum PublicFetchError {
    Source(SourceError),
    Network(NetError),
    HttpStatus(u16),
    InvalidResponse,
}
impl fmt::Display for PublicFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Network(error) => error.fmt(f),
            Self::HttpStatus(status) => write!(f, "Public download service returned HTTP {status}"),
            Self::InvalidResponse => {
                f.write_str("Public download service returned invalid metadata")
            }
        }
    }
}
impl std::error::Error for PublicFetchError {}
impl From<SourceError> for PublicFetchError {
    fn from(error: SourceError) -> Self {
        Self::Source(error)
    }
}

/// Poll both attempts together, returning the first success rather than the
/// first completion. Returning drops the losing future and its broker request.
async fn first_success<A, B, T, E>(first: A, second: B) -> Result<(usize, T), E>
where
    A: Future<Output = Result<T, E>>,
    B: Future<Output = Result<T, E>>,
{
    tokio::pin!(first, second);
    tokio::select! {
        result = &mut first => match result { Ok(value) => Ok((0, value)), Err(_) => second.await.map(|value| (1, value)) },
        result = &mut second => match result { Ok(value) => Ok((1, value)), Err(_) => first.await.map(|value| (0, value)) },
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SourceError {
    InvalidUrl,
    InvalidMirror,
}
impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidUrl => "invalid public artifact URL",
            Self::InvalidMirror => {
                "mirror must be an HTTPS base without credentials, query or fragment"
            }
        })
    }
}
impl std::error::Error for SourceError {}

impl DownloadSource {
    pub fn custom(base: &str) -> Result<Self, SourceError> {
        let base = Url::parse(base).map_err(|_| SourceError::InvalidMirror)?;
        validate_base(&base)?;
        Ok(Self::CustomBmclapi(base))
    }
    /// Short nonsecret label for request identity/provenance. A custom base is
    /// hashed, so URL text is not copied into downloader labels or logs.
    pub fn id(&self) -> String {
        match self {
            Self::Automatic => "automatic".into(),
            Self::Official => "official".into(),
            Self::Bmclapi => "bmclapi".into(),
            Self::CustomBmclapi(base) => format!(
                "custom-bmclapi-{:x}",
                Sha1::digest(base.as_str().as_bytes())
            ),
        }
    }
    pub fn attribution(&self) -> &str {
        match self {
            Self::Automatic => "Automatic selection between official downloads and BMCLAPI",
            Self::Official => "Mojang / Microsoft official downloads",
            Self::Bmclapi => "BMCLAPI (bangbang93) — https://bmclapidoc.bangbang93.com/",
            Self::CustomBmclapi(_) => "User-selected BMCLAPI-compatible mirror",
        }
    }
    /// The selected provider serves Mojang-format metadata and expected hashes.
    /// Unrecognized hosts and non-content paths are left untouched. This is a
    /// file-source adapter, not authorization for a mirror's cross-origin CDN.
    pub fn resolve(&self, official_url: &str) -> Result<String, SourceError> {
        if matches!(self, Self::Automatic) {
            return preferred_source().resolve(official_url);
        }
        let official = Url::parse(official_url).map_err(|_| SourceError::InvalidUrl)?;
        if matches!(self, Self::Official) {
            return Ok(official_url.into());
        }
        // Signed/authenticated endpoints never become mirror requests.
        if official.scheme() != "https"
            || !official.username().is_empty()
            || official.password().is_some()
            || official.port().is_some()
            || official.query().is_some()
            || official.fragment().is_some()
        {
            return Ok(official_url.into());
        }
        let path = if official.host_str() == Some("maven.neoforged.net") {
            official
                .path()
                .strip_prefix("/releases")
                .unwrap_or(official.path())
        } else {
            official.path()
        };
        let prefix = match official.host_str() {
            Some("resources.download.minecraft.net") => "/assets",
            Some("libraries.minecraft.net" | "maven.fabricmc.net" | "maven.minecraftforge.net") => {
                "/maven"
            }
            Some("maven.neoforged.net") if path.starts_with("/net/neoforged/neoforge/") => "/maven",
            Some("meta.fabricmc.net") if path.starts_with("/v2/") => "/fabric-meta",
            Some("piston-meta.mojang.com" | "launchermeta.mojang.com")
                if path.starts_with("/v1/packages/") || path.starts_with("/mc/game/") =>
            {
                ""
            }
            Some("piston-data.mojang.com" | "launcher.mojang.com")
                if path.starts_with("/v1/objects/") || path.starts_with("/mc/game/") =>
            {
                ""
            }
            _ => return Ok(official_url.into()),
        };
        let mut base = match self {
            Self::Bmclapi => Url::parse(BMCLAPI).map_err(|_| SourceError::InvalidMirror)?,
            Self::CustomBmclapi(base) => {
                validate_base(base)?;
                base.clone()
            }
            Self::Official | Self::Automatic => unreachable!("handled above"),
        };
        let mapped_path = format!("{}{prefix}{path}", base.path().trim_end_matches('/'));
        base.set_path(&mapped_path);
        Ok(base.into())
    }

    /// Concrete source identities and mapped URLs, without changing user settings.
    /// Unmapped/authenticated URLs are never raced or sent to another provider.
    pub fn candidates(
        &self,
        official_url: &str,
    ) -> Result<Vec<(DownloadSource, String)>, SourceError> {
        let first = if matches!(self, Self::Automatic) {
            preferred_source()
        } else {
            self.clone()
        };
        let resolved = first.resolve(official_url)?;
        if resolved == official_url {
            if matches!(self, Self::Automatic) {
                let mirror = Self::Bmclapi.resolve(official_url)?;
                if mirror != official_url {
                    return Ok(vec![
                        (Self::Official, official_url.into()),
                        (Self::Bmclapi, mirror),
                    ]);
                }
            }
            return Ok(vec![(Self::Official, official_url.into())]);
        }
        Ok(vec![
            (first, resolved),
            (Self::Official, official_url.into()),
        ])
    }

    /// Report the concrete attempted provider from `candidates`, never a URL.
    pub fn report_failure(&self) {
        let official = match self {
            Self::Official => true,
            Self::Bmclapi => false,
            _ => return,
        };
        if let Ok(mut preference) = PREFERENCE.lock() {
            preference.failed = Some((official, Instant::now()));
        }
    }

    fn report_success(&self) {
        let official = match self {
            Self::Official => true,
            Self::Bmclapi => false,
            _ => return,
        };
        if let Ok(mut preference) = PREFERENCE.lock() {
            preference.winner = Some((official, Instant::now()));
            if Preference::recent(preference.failed) == Some(official) {
                preference.failed = None;
            }
        }
    }

    /// Call only for explicit user work. Small public metadata uses short
    /// attempts; automatic selection races uncached providers. Larger binaries
    /// always use ordered attempts and the artifact-sized timeout.
    pub async fn fetch_public(
        &self,
        broker: &Broker,
        official_url: &str,
        limit: u64,
    ) -> Result<Vec<u8>, PublicFetchError> {
        self.fetch_public_validated(broker, official_url, limit, |bytes| Ok(bytes.to_vec()))
            .await
    }

    /// Validate inside each attempt, so an invalid HTTP 200 is not a winner.
    pub async fn fetch_public_validated<T, E, F>(
        &self,
        broker: &Broker,
        official_url: &str,
        limit: u64,
        parse: F,
    ) -> Result<T, E>
    where
        E: From<PublicFetchError>,
        F: Fn(&[u8]) -> Result<T, E> + Sync,
    {
        let candidates = self
            .candidates(official_url)
            .map_err(PublicFetchError::Source)
            .map_err(E::from)?;
        // An unmapped singleton API says nothing about the relative health of
        // official Minecraft content and its BMCLAPI alternative.
        let track_preference = candidates.len() == 2;
        let timeout = if limit <= METADATA_LIMIT {
            Duration::from_secs(8)
        } else {
            Duration::from_secs(300)
        };
        let cached = PREFERENCE
            .lock()
            .map(|preference| preference.cached())
            .unwrap_or(false);
        if matches!(self, Self::Automatic)
            && limit <= METADATA_LIMIT
            && candidates.len() == 2
            && !cached
        {
            let (index, value) = first_success(
                fetch_attempt(
                    &candidates[0],
                    broker,
                    limit,
                    timeout,
                    &parse,
                    track_preference,
                ),
                fetch_attempt(
                    &candidates[1],
                    broker,
                    limit,
                    timeout,
                    &parse,
                    track_preference,
                ),
            )
            .await?;
            candidates[index].0.report_success();
            return Ok(value);
        }
        let mut last = None;
        for candidate in &candidates {
            match fetch_attempt(candidate, broker, limit, timeout, &parse, track_preference).await {
                Ok(value) => {
                    if track_preference && matches!(self, Self::Automatic) {
                        candidate.0.report_success();
                    }
                    return Ok(value);
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last.unwrap_or_else(|| E::from(PublicFetchError::InvalidResponse)))
    }
}

async fn fetch_attempt<T, E, F>(
    candidate: &(DownloadSource, String),
    broker: &Broker,
    limit: u64,
    timeout: Duration,
    parse: &F,
    track_preference: bool,
) -> Result<T, E>
where
    E: From<PublicFetchError>,
    F: Fn(&[u8]) -> Result<T, E> + Sync,
{
    let result = async {
        let response = tokio::time::timeout(
            timeout,
            broker.get_public_metadata(&candidate.1, &candidate.0.id(), limit),
        )
        .await
        .map_err(|_| {
            E::from(PublicFetchError::Network(NetError::Timeout(
                TimeoutPhase::Total,
            )))
        })?
        .map_err(|error| E::from(PublicFetchError::Network(error)))?;
        if response.metadata.status != 200 {
            return Err(E::from(PublicFetchError::HttpStatus(
                response.metadata.status,
            )));
        }
        let parsed = parse(&response.body);
        if parsed.is_err()
            && let Some(cache) = cobble_net::PublicMetadataCache::shared()
        {
            let _ = cache
                .invalidate(&candidate.1, &candidate.0.id(), limit)
                .await;
        }
        parsed
    }
    .await;
    if track_preference && result.is_err() {
        candidate.0.report_failure();
    }
    result
}

fn preferred_source() -> DownloadSource {
    if PREFERENCE
        .lock()
        .map(|preference| preference.preferred())
        .unwrap_or(true)
    {
        DownloadSource::Official
    } else {
        DownloadSource::Bmclapi
    }
}

fn validate_base(base: &Url) -> Result<(), SourceError> {
    if base.scheme() != "https"
        || base.host_str().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Err(SourceError::InvalidMirror);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn race_keeps_waiting_after_fast_failure_for_slow_healthy_source() {
        let mut polls = 0;
        let healthy = std::future::poll_fn(|context| {
            polls += 1;
            if polls == 1 {
                context.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(Ok::<_, &str>("healthy"))
            }
        });
        assert_eq!(
            first_success(async { Err::<&str, _>("fast failure") }, healthy)
                .await
                .unwrap(),
            (1, "healthy")
        );
        assert_eq!(polls, 2);
        assert!(
            first_success(async { Err::<(), _>("a") }, async { Err::<(), _>("b") })
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn race_returns_fast_healthy_source_and_drops_losing_future() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Dropped(dropped.clone());
        let slow = async move {
            let _guard = guard;
            std::future::pending::<Result<&str, &str>>().await
        };
        assert_eq!(
            first_success(slow, async { Ok("fast") }).await.unwrap(),
            (1, "fast")
        );
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[test]
    fn preference_expires_and_recent_failure_deprioritizes_winner() {
        let mut preference = Preference {
            winner: Some((false, Instant::now())),
            failed: None,
        };
        assert!(!preference.preferred());
        assert!(preference.cached());
        preference.failed = Some((false, Instant::now()));
        assert!(preference.preferred());
        let old = Instant::now()
            .checked_sub(PREFERENCE_TTL + Duration::from_secs(1))
            .unwrap();
        preference.winner = Some((false, old));
        preference.failed = Some((true, old));
        assert!(!preference.cached());
        assert!(preference.preferred());
    }

    #[test]
    fn candidates_keep_manual_choices_and_auth_routes_separate() {
        let artifact = "https://libraries.minecraft.net/example.jar";
        let official = DownloadSource::Official.candidates(artifact).unwrap();
        assert_eq!(official.len(), 1);
        assert_eq!(official[0].0.id(), "official");
        let mirror = DownloadSource::Bmclapi.candidates(artifact).unwrap();
        assert_eq!(mirror.len(), 2);
        assert_eq!(mirror[0].0.id(), "bmclapi");
        assert_eq!(mirror[1].1, artifact);
        let automatic = DownloadSource::Automatic.candidates(artifact).unwrap();
        assert_eq!(automatic.len(), 2);
        assert!(
            automatic
                .iter()
                .any(|(source, _)| matches!(source, DownloadSource::Official))
        );
        assert!(
            automatic
                .iter()
                .any(|(source, _)| matches!(source, DownloadSource::Bmclapi))
        );
        for url in [
            "https://login.microsoftonline.com/oauth2/token",
            "https://libraries.minecraft.net/example.jar?token=secret",
            "https://example.invalid/unmapped.jar",
        ] {
            for source in [
                DownloadSource::Official,
                DownloadSource::Bmclapi,
                DownloadSource::Automatic,
            ] {
                let candidates = source.candidates(url).unwrap();
                assert_eq!(candidates.len(), 1);
                assert_eq!(candidates[0].1, url);
                assert_eq!(candidates[0].0.id(), "official");
            }
        }
    }

    #[test]
    fn maps_only_known_public_content_routes() {
        for (input, path) in [
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.21.1-52.1.0/forge-1.21.1-52.1.0-installer.jar",
                "/maven/net/minecraftforge/forge/1.21.1-52.1.0/forge-1.21.1-52.1.0-installer.jar",
            ),
            (
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.264/neoforge-21.1.264-installer.jar",
                "/maven/net/neoforged/neoforge/21.1.264/neoforge-21.1.264-installer.jar",
            ),
            (
                "https://meta.fabricmc.net/v2/versions/loader/1.21.1",
                "/fabric-meta/v2/versions/loader/1.21.1",
            ),
            (
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar",
                "/maven/net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar",
            ),
            (
                "https://libraries.minecraft.net/example/library.jar",
                "/maven/example/library.jar",
            ),
            (
                "https://resources.download.minecraft.net/ab/abcdef",
                "/assets/ab/abcdef",
            ),
            (
                "https://piston-data.mojang.com/v1/objects/hash/client.jar",
                "/v1/objects/hash/client.jar",
            ),
            (
                "https://piston-meta.mojang.com/v1/packages/hash/version.json",
                "/v1/packages/hash/version.json",
            ),
            (
                "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json",
                "/mc/game/version_manifest_v2.json",
            ),
        ] {
            assert_eq!(
                DownloadSource::Bmclapi.resolve(input).unwrap(),
                format!("https://bmclapi2.bangbang93.com{path}")
            );
            assert_eq!(DownloadSource::Official.resolve(input).unwrap(), input);
        }
        for input in [
            "https://login.microsoftonline.com/oauth2/token",
            "https://piston-meta.mojang.com/auth",
            "https://libraries.minecraft.net.evil.invalid/file",
            "https://libraries.minecraft.net/file?token=secret",
        ] {
            assert_eq!(DownloadSource::Bmclapi.resolve(input).unwrap(), input);
        }
    }

    #[test]
    fn custom_mirror_preserves_base_path_and_requires_explicit_https() {
        let source = DownloadSource::custom("https://mirror.example.invalid/minecraft/").unwrap();
        assert_eq!(
            source
                .resolve("https://libraries.minecraft.net/a.jar")
                .unwrap(),
            "https://mirror.example.invalid/minecraft/maven/a.jar"
        );
        assert!(source.id().starts_with("custom-bmclapi-"));
        for base in [
            "http://mirror.example.invalid/",
            "https://user:password@mirror.example.invalid/",
            "https://mirror.example.invalid/?token=secret",
            "https://mirror.example.invalid/#fragment",
        ] {
            assert!(DownloadSource::custom(base).is_err());
        }
    }
}

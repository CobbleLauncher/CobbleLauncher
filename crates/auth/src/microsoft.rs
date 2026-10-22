// SPDX-License-Identifier: MIT
//! Native-only authentication state. Public profile JSON never contains secrets.
use crate::protocol::{self, AuthError, Http, Session, TokenPoll, Transport};
use crate::vault::{SystemVault, Vault};
use crate::{
    BoxError, OfflineProfile, PlayerProfile, ProfileLibrary, find_profile, get_player_profile,
    save_library_at, with_library,
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize)]
pub struct DeviceLogin {
    pub flow_id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LoginPoll {
    Pending,
    Complete { library: ProfileLibrary },
}

/// Core-only launch snapshot. No Debug/Serialize implementation exposes tokens.
#[derive(Clone)]
pub struct LaunchIdentity {
    pub name: String,
    pub uuid: String,
    pub user_type: String,
    pub xuid: String,
    access_token: String,
}
impl LaunchIdentity {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }
    pub fn offline(profile: &OfflineProfile) -> Result<Self, BoxError> {
        protocol::offline_valid(profile)?;
        Ok(Self {
            name: profile.name.clone(),
            uuid: profile.uuid.clone(),
            user_type: "legacy".into(),
            xuid: String::new(),
            access_token: "0".into(),
        })
    }
    fn from_session(session: &Session) -> Self {
        Self {
            name: session.name.clone(),
            uuid: session.uuid.clone(),
            user_type: "msa".into(),
            xuid: session.xuid.clone(),
            access_token: session.token.clone(),
        }
    }
}

struct Cached {
    identity: LaunchIdentity,
    expires: Instant,
}
struct Flow {
    id: String,
    device_code: String,
    verification_uri: String,
    expires: Instant,
    next_poll: Instant,
    interval: Duration,
    busy: bool,
}
struct State {
    generation: Option<String>,
    flow: Option<Flow>,
    cache: BTreeMap<String, Cached>,
}
static STATE: Mutex<State> = Mutex::new(State {
    generation: None,
    flow: None,
    cache: BTreeMap::new(),
});
// A small single-flight gate serializes exchanges/refreshes. Profile removal is
// synchronous and wins through the profile/state checks before persistence.
static AUTH_OPERATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

impl State {
    fn cancel_flow(&mut self, id: &str) {
        if self.generation.as_deref() == Some(id) {
            self.cancel();
        }
    }
    fn allow_commit(&self, id: &str) -> Result<(), AuthError> {
        if self.generation.as_deref() != Some(id)
            || self.flow.as_ref().is_none_or(|flow| flow.id != id)
        {
            return Err(AuthError::Cancelled);
        }
        if self
            .flow
            .as_ref()
            .is_some_and(|flow| Instant::now() >= flow.expires)
        {
            return Err(AuthError::Expired);
        }
        Ok(())
    }
    fn cancel(&mut self) {
        self.generation = None;
        self.flow = None;
    }
}

fn random_id() -> Result<String, AuthError> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::InvalidResponse)?;
    Ok(uuid::Uuid::from_bytes(bytes).simple().to_string())
}

pub async fn begin_microsoft_login() -> Result<DeviceLogin, BoxError> {
    let id = random_id()?;
    {
        let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
        state.generation = Some(id.clone());
        state.flow = None;
    }
    let _operation = AUTH_OPERATION.lock().await;
    let response = tokio::task::spawn_blocking(|| protocol::device(&Http))
        .await
        .map_err(|_| AuthError::Network)??;
    let now = Instant::now();
    let public = DeviceLogin {
        flow_id: id.clone(),
        user_code: response.user_code,
        verification_uri: response.verification_uri.clone(),
        expires_in: response.expires_in,
        interval: response.interval,
    };
    let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
    if state.generation.as_deref() != Some(&id) {
        return Err(AuthError::Cancelled.into());
    }
    state.flow = Some(Flow {
        id,
        device_code: response.device_code,
        verification_uri: response.verification_uri,
        expires: now + Duration::from_secs(response.expires_in),
        next_poll: now + Duration::from_secs(response.interval),
        interval: Duration::from_secs(response.interval),
        busy: false,
    });
    Ok(public)
}

pub fn microsoft_login_url(flow_id: &str) -> Result<String, BoxError> {
    let state = STATE.lock().map_err(|_| AuthError::Storage)?;
    let flow = state
        .flow
        .as_ref()
        .filter(|flow| flow.id == flow_id)
        .ok_or(AuthError::Cancelled)?;
    if Instant::now() >= flow.expires {
        return Err(AuthError::Expired.into());
    }
    protocol::verification_uri(&flow.verification_uri)?;
    Ok(flow.verification_uri.clone())
}

pub fn cancel_microsoft_login(flow_id: &str) -> Result<(), BoxError> {
    let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
    state.cancel_flow(flow_id);
    Ok(())
}

impl Flow {
    fn reserve(&mut self, now: Instant) -> Result<Option<String>, AuthError> {
        if now >= self.expires {
            return Err(AuthError::Expired);
        }
        if self.busy || now < self.next_poll {
            return Ok(None);
        }
        self.busy = true;
        self.next_poll = now + self.interval;
        Ok(Some(self.device_code.clone()))
    }
    fn pending(&mut self, slow_down: bool) {
        if slow_down {
            self.interval += Duration::from_secs(5);
        }
        self.next_poll = Instant::now() + self.interval;
        self.busy = false;
    }
}

pub async fn poll_microsoft_login(flow_id: &str) -> Result<LoginPoll, BoxError> {
    let code = {
        let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
        let flow = state
            .flow
            .as_mut()
            .filter(|flow| flow.id == flow_id)
            .ok_or(AuthError::Cancelled)?;
        flow.reserve(Instant::now())?
    };
    let Some(code) = code else {
        return Ok(LoginPoll::Pending);
    };
    let id = flow_id.to_owned();
    let _operation = AUTH_OPERATION.lock().await;
    let worker_id = id.clone();
    let result = tokio::task::spawn_blocking(move || poll_worker(&worker_id, &code))
        .await
        .unwrap_or_else(|_| Err(AuthError::Network.into()));
    if let Err(error) = &result {
        let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
        if state.generation.as_deref() == Some(&id) {
            if matches!(
                error.downcast_ref::<AuthError>(),
                Some(
                    AuthError::Network
                        | AuthError::Service {
                            status: 429 | 500..=599,
                            ..
                        }
                )
            ) {
                if let Some(flow) = state.flow.as_mut() {
                    flow.busy = false;
                }
            } else {
                state.generation = None;
                state.flow = None;
            }
        }
    }
    result
}

fn poll_worker(id: &str, code: &str) -> Result<LoginPoll, BoxError> {
    {
        let state = STATE.lock().map_err(|_| AuthError::Storage)?;
        state.allow_commit(id)?;
    }
    let http = FlowHttp { id };
    match protocol::poll(&http, code)? {
        TokenPoll::Pending => pending_poll(id, false),
        TokenPoll::SlowDown => pending_poll(id, true),
        TokenPoll::Complete(token) => {
            let refresh = token
                .refresh_token
                .filter(|token| !token.is_empty())
                .ok_or(AuthError::ReauthenticationRequired)?;
            let session = protocol::exchange(&http, &token.access_token)?;
            let library = with_library(|directory, library| {
                let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
                state.allow_commit(id)?;
                let library = persist(directory, library, &session, &refresh, &SystemVault)?;
                cache(&mut state, &session);
                state.generation = None;
                state.flow = None;
                Ok(library)
            })?;
            Ok(LoginPoll::Complete { library })
        }
    }
}

struct FlowHttp<'a> {
    id: &'a str,
}
impl Transport for FlowHttp<'_> {
    fn send(
        &self,
        endpoint: protocol::Endpoint,
        body: Option<(&str, bool)>,
        bearer: Option<&str>,
    ) -> Result<protocol::Response, AuthError> {
        STATE
            .lock()
            .map_err(|_| AuthError::Storage)?
            .allow_commit(self.id)?;
        Http.send(endpoint, body, bearer)
    }
}

fn pending_poll(id: &str, slow_down: bool) -> Result<LoginPoll, BoxError> {
    let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
    let flow = state
        .flow
        .as_mut()
        .filter(|flow| flow.id == id)
        .ok_or(AuthError::Cancelled)?;
    if Instant::now() >= flow.expires {
        return Err(AuthError::Expired.into());
    }
    flow.pending(slow_down);
    Ok(LoginPoll::Pending)
}

fn cache(state: &mut State, session: &Session) {
    state.cache.insert(
        format!("microsoft-{}", session.uuid),
        Cached {
            identity: LaunchIdentity::from_session(session),
            expires: Instant::now() + Duration::from_secs(session.expires_in.saturating_sub(60)),
        },
    );
}
fn cached(id: &str) -> Result<Option<LaunchIdentity>, AuthError> {
    let state = STATE.lock().map_err(|_| AuthError::Storage)?;
    Ok(state
        .cache
        .get(id)
        .filter(|cached| Instant::now() < cached.expires)
        .map(|cached| cached.identity.clone()))
}

fn persist(
    directory: &Path,
    mut library: ProfileLibrary,
    session: &Session,
    refresh: &str,
    vault: &impl Vault,
) -> Result<ProfileLibrary, AuthError> {
    let id = format!("microsoft-{}", session.uuid);
    if let Some(profile) = library.profiles.iter_mut().find(|profile| profile.id == id) {
        if profile.kind != "microsoft" || profile.uuid != session.uuid {
            return Err(AuthError::InvalidResponse);
        }
        profile.name = session.name.clone();
    } else {
        library.profiles.push(PlayerProfile {
            id: id.clone(),
            kind: "microsoft".into(),
            name: session.name.clone(),
            uuid: session.uuid.clone(),
            note: None,
        });
    }
    library.active_id = Some(id.clone());
    let previous = vault.get(&id)?;
    vault.set(&id, refresh)?;
    if save_library_at(directory, &library).is_err() {
        match previous {
            Some(previous) => vault.set(&id, &previous)?,
            None => vault.delete(&id)?,
        };
        return Err(AuthError::Storage);
    }
    Ok(library)
}

pub async fn launch_identity(profile_id: Option<&str>) -> Result<LaunchIdentity, BoxError> {
    let profile = get_player_profile(profile_id)?;
    if profile.kind == "offline" {
        return LaunchIdentity::offline(&OfflineProfile {
            name: profile.name,
            uuid: profile.uuid,
        });
    }
    if profile.kind != "microsoft" {
        return Err(AuthError::InvalidResponse.into());
    }
    if let Some(identity) = cached(&profile.id)? {
        return Ok(identity);
    }
    let _operation = AUTH_OPERATION.lock().await;
    if let Some(identity) = cached(&profile.id)? {
        return Ok(identity);
    }
    tokio::task::spawn_blocking(move || {
        let (session, refresh) = refreshed_session(&profile, &Http, &SystemVault)?;
        with_library(|directory, mut library| {
            find_profile(&library, &profile.id)?;
            let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
            // Preserve the user's current selection; refreshing one bound game
            // must not switch the launcher's active player profile.
            let active = library.active_id.clone();
            let current = library
                .profiles
                .iter_mut()
                .find(|item| item.id == profile.id)
                .ok_or(AuthError::ReauthenticationRequired)?;
            if current.kind != "microsoft" || current.uuid != profile.uuid {
                return Err(AuthError::ReauthenticationRequired.into());
            }
            current.name = session.name.clone();
            SystemVault.set(&profile.id, &refresh)?;
            library.active_id = active;
            save_library_at(directory, &library)?;
            let identity = LaunchIdentity::from_session(&session);
            cache(&mut state, &session);
            Ok(identity)
        })
    })
    .await
    .map_err(|_| AuthError::Network)?
}

fn refreshed_session(
    profile: &PlayerProfile,
    http: &impl Transport,
    vault: &impl Vault,
) -> Result<(Session, String), AuthError> {
    let refresh = vault
        .get(&profile.id)?
        .filter(|token| !token.is_empty())
        .ok_or(AuthError::ReauthenticationRequired)?;
    let token = protocol::refresh(http, &refresh)?;
    let session = protocol::exchange(http, &token.access_token)?;
    if session.uuid != profile.uuid {
        return Err(AuthError::ReauthenticationRequired);
    }
    Ok((
        session,
        token
            .refresh_token
            .filter(|token| !token.is_empty())
            .unwrap_or(refresh),
    ))
}

/// Called while holding the profile-library lock; always acquire state second.
pub(super) fn revoke(id: &str) -> Result<(), AuthError> {
    let mut state = STATE.lock().map_err(|_| AuthError::Storage)?;
    SystemVault.delete(id)?;
    state.cache.remove(id);
    state.cancel();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Endpoint, fixtures::Fixture};
    use serde_json::json;
    #[derive(Default)]
    struct FakeVault {
        tokens: Mutex<BTreeMap<String, String>>,
        fail: bool,
    }
    impl Vault for FakeVault {
        fn get(&self, id: &str) -> Result<Option<String>, AuthError> {
            if self.fail {
                return Err(AuthError::VaultUnavailable);
            }
            Ok(self.tokens.lock().unwrap().get(id).cloned())
        }
        fn set(&self, id: &str, token: &str) -> Result<(), AuthError> {
            if self.fail {
                return Err(AuthError::VaultUnavailable);
            }
            self.tokens.lock().unwrap().insert(id.into(), token.into());
            Ok(())
        }
        fn delete(&self, id: &str) -> Result<(), AuthError> {
            if self.fail {
                return Err(AuthError::VaultUnavailable);
            }
            self.tokens.lock().unwrap().remove(id);
            Ok(())
        }
    }
    fn session() -> Session {
        Session {
            name: "OnlineName".into(),
            uuid: "0123456789abcdef0123456789abcdef".into(),
            token: "synthetic-minecraft-secret".into(),
            xuid: "12345".into(),
            expires_in: 86400,
        }
    }
    fn profile() -> PlayerProfile {
        let session = session();
        PlayerProfile {
            id: format!("microsoft-{}", session.uuid),
            kind: "microsoft".into(),
            name: session.name,
            uuid: session.uuid,
            note: Some("keep note".into()),
        }
    }

    #[test]
    fn online_profile_persistence_keeps_secrets_in_vault_and_reauthentication_preserves_id_and_note()
     {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault::default();
        let mut library = ProfileLibrary::default();
        crate::add_to_library(&mut library, "OnlineName", None).unwrap();
        let mut library = persist(
            directory.path(),
            library,
            &session(),
            "synthetic-refresh-secret",
            &vault,
        )
        .unwrap();
        assert_eq!(library.profiles.len(), 2);
        let id = profile().id;
        assert_eq!(library.active_id.as_deref(), Some(id.as_str()));
        assert_eq!(
            vault.get(&id).unwrap().as_deref(),
            Some("synthetic-refresh-secret")
        );
        library
            .profiles
            .iter_mut()
            .find(|profile| profile.id == id)
            .unwrap()
            .note = Some("keep note".into());
        let mut renamed = session();
        renamed.name = "RenamedPlayer".into();
        let library = persist(
            directory.path(),
            library,
            &renamed,
            "synthetic-rotated-refresh",
            &vault,
        )
        .unwrap();
        assert_eq!(library.profiles.len(), 2);
        let saved = library
            .profiles
            .iter()
            .find(|profile| profile.id == id)
            .unwrap();
        assert_eq!(saved.name, "RenamedPlayer");
        assert_eq!(saved.note.as_deref(), Some("keep note"));
        let public =
            std::fs::read_to_string(directory.path().join("player-profiles.json")).unwrap();
        assert!(!public.contains("synthetic-"));
        assert!(crate::resolve_from_library(&library, Some(&id)).is_err());
        assert_eq!(crate::read_library_at(directory.path()).unwrap(), library);
    }

    #[test]
    fn vault_failure_does_not_save_public_profile_and_file_failure_restores_previous_refresh_token()
    {
        let directory = tempfile::tempdir().unwrap();
        let vault = FakeVault {
            fail: true,
            ..FakeVault::default()
        };
        assert!(matches!(
            persist(
                directory.path(),
                ProfileLibrary::default(),
                &session(),
                "synthetic-refresh",
                &vault
            ),
            Err(AuthError::VaultUnavailable)
        ));
        assert!(!directory.path().join("player-profiles.json").exists());
        let vault = FakeVault::default();
        let id = profile().id;
        vault.set(&id, "synthetic-old-refresh").unwrap();
        std::fs::create_dir(directory.path().join("player-profiles.json")).unwrap();
        assert!(matches!(
            persist(
                directory.path(),
                ProfileLibrary::default(),
                &session(),
                "synthetic-new-refresh",
                &vault
            ),
            Err(AuthError::Storage)
        ));
        assert_eq!(
            vault.get(&id).unwrap().as_deref(),
            Some("synthetic-old-refresh")
        );
    }

    #[test]
    fn refresh_uses_vault_and_rotation_checks_account_binding_without_any_offline_fallback() {
        let vault = FakeVault::default();
        let profile = profile();
        vault
            .set(&profile.id, "synthetic-original-refresh")
            .unwrap();
        let mut responses = vec![(
            Endpoint::MicrosoftToken,
            200,
            json!({"access_token":"synthetic-ms-refreshed","refresh_token":"synthetic-rotated-refresh"}),
        )];
        responses.extend(Fixture::chain());
        let http = Fixture::new(responses);
        let (identity, next) = refreshed_session(&profile, &http, &vault).unwrap();
        assert_eq!(identity.uuid, profile.uuid);
        assert_eq!(next, "synthetic-rotated-refresh");
        let calls = http.calls.lock().unwrap();
        let fields: std::collections::HashMap<_, _> =
            url::form_urlencoded::parse(calls[0].1.as_bytes())
                .into_owned()
                .collect();
        assert_eq!(fields["grant_type"], "refresh_token");
        assert_eq!(fields["refresh_token"], "synthetic-original-refresh");
        drop(calls);
        assert!(matches!(
            refreshed_session(&profile, &Fixture::new(Vec::new()), &FakeVault::default()),
            Err(AuthError::ReauthenticationRequired)
        ));
        let mut wrong = profile.clone();
        wrong.uuid = "fedcba9876543210fedcba9876543210".into();
        let mut responses = vec![(
            Endpoint::MicrosoftToken,
            200,
            json!({"access_token":"synthetic-ms-refreshed"}),
        )];
        responses.extend(Fixture::chain());
        assert!(matches!(
            refreshed_session(&wrong, &Fixture::new(responses), &vault),
            Err(AuthError::ReauthenticationRequired)
        ));
    }

    #[test]
    fn interval_slowdown_busy_expiration_and_cancel_block_late_commit() {
        let now = Instant::now();
        let mut flow = Flow {
            id: "flow".into(),
            device_code: "synthetic-device".into(),
            verification_uri: "https://microsoft.com/devicelogin".into(),
            expires: now + Duration::from_secs(60),
            next_poll: now + Duration::from_secs(5),
            interval: Duration::from_secs(5),
            busy: false,
        };
        assert!(flow.reserve(now).unwrap().is_none());
        assert!(
            flow.reserve(now + Duration::from_secs(6))
                .unwrap()
                .is_some()
        );
        assert!(
            flow.reserve(now + Duration::from_secs(12))
                .unwrap()
                .is_none()
        );
        flow.pending(true);
        assert_eq!(flow.interval, Duration::from_secs(10));
        assert!(matches!(
            flow.reserve(now + Duration::from_secs(61)),
            Err(AuthError::Expired)
        ));
        let mut state = State {
            generation: Some("flow".into()),
            flow: Some(flow),
            cache: BTreeMap::new(),
        };
        assert!(state.allow_commit("flow").is_ok());
        state.cancel();
        assert_eq!(state.allow_commit("flow"), Err(AuthError::Cancelled));
        assert_eq!(state.allow_commit("other"), Err(AuthError::Cancelled));
    }

    #[test]
    fn launch_snapshots_keep_offline_and_microsoft_identity_types_separate_and_cache_expires() {
        let offline = OfflineProfile::new("Player").unwrap();
        let identity = LaunchIdentity::offline(&offline).unwrap();
        assert_eq!(identity.user_type, "legacy");
        assert_eq!(identity.access_token(), "0");
        let online = LaunchIdentity::from_session(&session());
        assert_eq!(online.user_type, "msa");
        assert_eq!(online.access_token(), "synthetic-minecraft-secret");
        let mut state = State {
            generation: None,
            flow: None,
            cache: BTreeMap::new(),
        };
        cache(&mut state, &session());
        let cached = state.cache.get(&profile().id).unwrap();
        assert!(cached.expires > Instant::now());
        assert_eq!(cached.identity.uuid, profile().uuid);
    }

    #[test]
    fn cancellation_is_idempotent_after_completion_and_does_not_cancel_a_new_flow() {
        let mut state = State {
            generation: None,
            flow: None,
            cache: BTreeMap::new(),
        };
        state.cancel_flow("completed");
        assert!(state.generation.is_none());
        state.generation = Some("new-flow".into());
        state.cancel_flow("old-flow");
        assert_eq!(state.generation.as_deref(), Some("new-flow"));
        state.cancel_flow("new-flow");
        state.cancel_flow("new-flow");
        assert!(state.generation.is_none());
    }
}

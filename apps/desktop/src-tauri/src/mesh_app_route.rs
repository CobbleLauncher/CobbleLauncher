// SPDX-License-Identifier: MIT
//! Host-only public application routing and game reservation arbitration.
use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};

static TRANSITION: AtomicBool = AtomicBool::new(false);
static PAUSED: AtomicBool = AtomicBool::new(false);
static GAMES: AtomicUsize = AtomicUsize::new(0);
#[derive(Clone, PartialEq, Serialize)]
struct Route {
    relay: String,
    route_policy: overview::RoutingPolicy,
}
struct Applied {
    generation: u64,
    route: Option<Route>,
    confirmed: bool,
}
fn matches(applied: &Option<Applied>, generation: u64, route: &Option<Route>) -> bool {
    applied.as_ref().is_some_and(|current| {
        current.confirmed && current.generation == generation && &current.route == route
    })
}
fn fresh_lan(applied: &Option<Applied>, generation: u64, route: &Option<Route>) -> bool {
    route.is_none()
        && applied
            .as_ref()
            .is_none_or(|current| current.generation != generation)
}
enum Job {
    Reconcile(Option<mpsc::Sender<Result<(), String>>>),
}
fn jobs() -> &'static mpsc::SyncSender<Job> {
    static JOBS: OnceLock<mpsc::SyncSender<Job>> = OnceLock::new();
    JOBS.get_or_init(|| {
        let (sender, receiver) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            let mut applied: Option<Applied> = None;
            for Job::Reconcile(reply) in receiver {
                let result = reconcile(&mut applied, reply.is_some());
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
        });
        sender
    })
}
pub(super) fn schedule() {
    let _ = jobs().try_send(Job::Reconcile(None));
}
pub(super) fn reconcile_now() -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    jobs()
        .try_send(Job::Reconcile(Some(sender)))
        .map_err(|_| "MESH_EVENT_BUSY")?;
    receiver
        .recv_timeout(Duration::from_secs(25))
        .map_err(|_| "MESH_EVENT_ROUTE_FAILED")?
}
pub(super) fn changing() -> bool {
    TRANSITION.load(Ordering::Acquire)
}
pub(super) fn paused() -> bool {
    PAUSED.load(Ordering::Acquire)
}
pub(super) struct GamePause;
pub(super) fn pause_for_game() -> Result<GamePause, String> {
    GAMES.fetch_add(1, Ordering::AcqRel);
    if let Err(error) = reconcile_now() {
        GAMES.fetch_sub(1, Ordering::AcqRel);
        schedule();
        return Err(error);
    }
    Ok(GamePause)
}
impl Drop for GamePause {
    fn drop(&mut self) {
        GAMES.fetch_sub(1, Ordering::AcqRel);
        schedule();
    }
}
/// Existing session watchers trigger recovery on every terminal path, including errors.
pub(super) struct RestoreOnExit;
impl Drop for RestoreOnExit {
    fn drop(&mut self) {
        schedule();
    }
}

fn selected(
    saved: overview::RelaySelection,
    services: &[overview::ConfiguredService],
) -> Option<Route> {
    if let Some(relay) = saved.address.filter(|relay| overview::valid_relay(relay)) {
        return Some(Route {
            relay,
            route_policy: saved.route_policy,
        });
    }
    services
        .iter()
        .find(|service| {
            service.kind == "relay"
                && service.state == "authenticated"
                && overview::valid_relay(&service.endpoint)
        })
        .map(|service| Route {
            relay: service.endpoint.clone(),
            route_policy: overview::RoutingPolicy::default(),
        })
}
fn desired() -> Option<Route> {
    let network = network::mesh_network_snapshot().ok();
    let services = network
        .as_ref()
        .filter(|snapshot| snapshot.enabled && snapshot.phase == "online")
        .map(|snapshot| snapshot.services.as_slice())
        .unwrap_or_default();
    selected(overview::saved_relay(), services)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configured {
    changed: bool,
    generation: u64,
    snapshot: app::AppSnapshot,
}
fn reconcile(applied: &mut Option<Applied>, explicit_retry: bool) -> Result<(), String> {
    let wanted = desired();
    let (client, generation, game_active) = {
        let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        if enabled::require(&mut state).is_err() {
            *applied = None;
            PAUSED.store(false, Ordering::Release);
            return Ok(());
        }
        let Some(client) = state
            .social_client
            .as_ref()
            .filter(|client| client.alive())
            .cloned()
        else {
            *applied = None;
            PAUSED.store(false, Ordering::Release);
            return Ok(());
        };
        let game = GAMES.load(Ordering::Acquire) > 0
            || state
                .binding
                .as_ref()
                .is_some_and(|binding| active(&binding.public.state))
            || groups::occupied();
        (client, state.social_generation, game)
    };
    PAUSED.store(game_active && wanted.is_some(), Ordering::Release);
    let target = if game_active { None } else { wanted };
    if matches(applied, generation, &target) {
        if !explicit_retry || target.is_none() {
            return Ok(());
        }
        // A user retry must be able to restart a failed same-config actor. A
        // background status update must not create an independent retry loop.
        let status: app::AppSnapshot = serde_json::from_value(
            client
                .request(json!({"op":"app_status"}))
                .map_err(route_error)?,
        )
        .map_err(|_| "MESH_INVALID_RESPONSE")?;
        app::validate_snapshot(&status)?;
        if !status.event_route_failed() {
            return Ok(());
        }
    }
    // All native-created services start in LAN mode. A fresh identity needs no
    // destructive LAN-to-LAN rebuild and must not expire unrelated LAN listeners.
    if fresh_lan(applied, generation, &target) {
        *applied = Some(Applied {
            generation,
            route: None,
            confirmed: true,
        });
        return Ok(());
    }
    TRANSITION.store(true, Ordering::Release);
    struct Transition;
    impl Drop for Transition {
        fn drop(&mut self) {
            TRANSITION.store(false, Ordering::Release);
        }
    }
    let _transition = Transition;
    let generation = {
        let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
        enabled::require(&mut state)?;
        if state.social_generation != generation {
            return Err("MESH_IDENTITY_CHANGED".into());
        }
        state.social_generation = state.social_generation.wrapping_add(1);
        state.social_generation
    };
    crate::plugins::plugin_mesh_events::reset_generation(generation);
    // Unknown is different from a confirmed LAN mode. Even if stopping the owned
    // helper fails, a future game pause must explicitly confirm a null route.
    *applied = Some(Applied {
        generation,
        route: None,
        confirmed: false,
    });
    // Never hold the broker mutex while the companion stops/reserves a route.
    let command = match &target {
        Some(route) => {
            json!({"op":"app_route_configure","relay":route.relay,"route_policy":route.route_policy})
        }
        None => json!({"op":"app_route_configure","relay":null}),
    };
    let response = client.request(command);
    let rejected = response
        .as_ref()
        .is_err_and(|error| confirmed_route_rejection(error));
    if response
        .as_ref()
        .is_err_and(|error| !confirmed_route_rejection(error))
    {
        // A timed-out or malformed control channel cannot prove that old relay
        // authority was revoked. Stop only this owned helper and let the existing
        // network lifecycle reconnect it; never retain an uncertain old route.
        client.shutdown();
    }
    let response = response.map_err(route_error).and_then(|value| {
        let response: Configured =
            serde_json::from_value(value).map_err(|_| "MESH_INVALID_RESPONSE")?;
        app::validate_snapshot(&response.snapshot)?;
        if response.snapshot.events_public() != target.is_some() {
            return Err("MESH_INVALID_RESPONSE".into());
        }
        Ok(response)
    });
    if response
        .as_ref()
        .is_err_and(|error| error == "MESH_INVALID_RESPONSE")
    {
        client.shutdown();
    }
    let mut state = broker().lock().map_err(|_| "MESH_STATE_UNAVAILABLE")?;
    enabled::require(&mut state)?;
    if state.social_generation != generation {
        return Err("MESH_IDENTITY_CHANGED".into());
    }
    // Failed reconfiguration is fail-closed in the companion; do not cache the
    // rejected policy as applied or revive handles from the old public service.
    *applied = Some(Applied {
        generation,
        route: None,
        confirmed: rejected,
    });
    let response = response?;
    let _receipt = (response.changed, response.generation);
    *applied = Some(Applied {
        generation,
        route: target,
        confirmed: true,
    });
    Ok(())
}
fn confirmed_route_rejection(error: &str) -> bool {
    matches!(
        error,
        "MESH_EVENT_ROUTE_GAME_ACTIVE"
            | "MESH_EVENT_ROUTE_ACTIVE"
            | "MESH_EVENT_ROUTE_NOT_AUTHORIZED"
            | "MESH_EVENT_DIRECT_REQUIRED"
            | "MESH_EVENT_ROUTE_FAILED"
            | "MESH_EVENT_ROUTE_NOT_READY"
    )
}
fn route_error(error: String) -> String {
    match error.as_str() {
        "MESH_DISABLED"
        | "MESH_IDENTITY_CHANGED"
        | "MESH_EVENT_ROUTE_GAME_ACTIVE"
        | "MESH_EVENT_ROUTE_ACTIVE"
        | "MESH_EVENT_ROUTE_NOT_AUTHORIZED"
        | "MESH_EVENT_DIRECT_REQUIRED"
        | "MESH_EVENT_ROUTE_FAILED" => error,
        _ => "MESH_EVENT_ROUTE_FAILED".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uncertain_configuration_never_satisfies_game_lan_barrier() {
        let unknown = Some(Applied {
            generation: 4,
            route: None,
            confirmed: false,
        });
        assert!(!matches(&unknown, 4, &None));
        assert!(!fresh_lan(&unknown, 4, &None));
        let confirmed = Some(Applied {
            generation: 4,
            route: None,
            confirmed: true,
        });
        assert!(matches(&confirmed, 4, &None));
        assert!(fresh_lan(&None, 4, &None));
        assert!(!confirmed_route_rejection("MESH_APP_BUSY"));
        assert!(!confirmed_route_rejection("timeout after successful write"));
        assert!(confirmed_route_rejection("MESH_EVENT_ROUTE_NOT_AUTHORIZED"));
    }
    fn saved(
        address: Option<&str>,
        authorization: overview::RoutingAuthorization,
    ) -> overview::RelaySelection {
        overview::RelaySelection {
            address: address.map(str::to_owned),
            source: None,
            state: "configured",
            route_policy: overview::RoutingPolicy {
                authorization,
                observer: None,
            },
        }
    }
    #[test]
    fn explicit_local_policy_is_never_inferred_from_a_signed_seed() {
        let service = overview::ConfiguredService {
            id: "seed".into(),
            kind: "relay".into(),
            endpoint: "/dns4/seed.example/tcp/443/p2p/seed".into(),
            state: "authenticated".into(),
            discovery: "not_checked".into(),
            game_relay: true,
        };
        let route = selected(
            saved(None, overview::RoutingAuthorization::LegacyTest),
            std::slice::from_ref(&service),
        )
        .unwrap();
        assert!(matches!(
            route.route_policy.authorization,
            overview::RoutingAuthorization::Coordination
        ));
        let route = selected(
            saved(
                Some("/dns4/saved.example/tcp/443/p2p/saved"),
                overview::RoutingAuthorization::LegacyTest,
            ),
            &[service],
        )
        .unwrap();
        assert!(matches!(
            route.route_policy.authorization,
            overview::RoutingAuthorization::LegacyTest
        ));
    }
    #[test]
    fn unavailable_or_unverified_services_do_not_provision_public_routes() {
        assert!(
            selected(
                saved(None, overview::RoutingAuthorization::Coordination),
                &[]
            )
            .is_none()
        );
        let service = overview::ConfiguredService {
            id: "seed".into(),
            kind: "relay".into(),
            endpoint: "/dns4/seed.example/tcp/443/p2p/seed".into(),
            state: "configured".into(),
            discovery: "available".into(),
            game_relay: true,
        };
        assert!(
            selected(
                saved(None, overview::RoutingAuthorization::Coordination),
                &[service]
            )
            .is_none()
        );
        assert_eq!(
            route_error("PRIVATE peer/name/body".into()),
            "MESH_EVENT_ROUTE_FAILED"
        );
    }
}

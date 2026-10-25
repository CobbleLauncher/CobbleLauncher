// SPDX-License-Identifier: MIT
use super::*;

#[test]
fn cancellation_before_dispatch_does_not_start_resolution() {
    let mut store = PreviewStore::default();
    store.cancel("not-polled-yet");
    assert!(store.begin("not-polled-yet".into()).is_err());
    assert!(store.active.is_none());
    assert!(store.begin("fresh-request".into()).is_ok());
}

fn choice(loader: &str, version: &str) -> ReverseConfigurationChoice {
    ReverseConfigurationChoice {
        game_version: version.into(),
        loader: loader.into(),
        roots: vec![cobble_content::ModVersionSelection {
            project_id: "root".into(),
            version_id: "v1".into(),
        }],
        plan: cobble_content::ModrinthPlan {
            project_id: "root".into(),
            version_id: "v1".into(),
            mods: Vec::new(),
        },
    }
}

#[test]
fn plan_claim_is_single_use_and_retryable_after_loader_failure() {
    let mut store = PreviewStore::default();
    store
        .plans
        .insert("plan".into(), (Instant::now(), choice("fabric", "1.20.1")));
    assert!(store.claim("renderer-invented").is_err());
    assert!(store.claim("plan").is_ok());
    assert!(store.claim("plan").is_err());
    store.release("plan", false);
    assert!(store.claim("plan").is_ok());
    store.release("plan", true);
    assert!(store.claim("plan").is_err());
}

#[test]
fn expired_plan_cannot_create_game() {
    let mut store = PreviewStore::default();
    store.plans.insert(
        "old".into(),
        (
            Instant::now() - PLAN_LIFETIME - Duration::from_secs(1),
            choice("fabric", "1.20.1"),
        ),
    );
    assert_eq!(store.claim("old").unwrap_err(), "REVERSE_PLAN_EXPIRED");
}

#[test]
fn unsupported_installer_choices_never_receive_install_tokens() {
    let mut store = PreviewStore::default();
    let _receiver = store.begin("request".into()).unwrap();
    let result = publish(
        &mut store,
        "request",
        ReverseConfiguration {
            choices: vec![choice("fabric", "1.20.1"), choice("quilt", "1.20.1")],
            conflicts: vec![],
            stats: ReverseConfigurationStats::default(),
            limitations: vec![],
        },
    )
    .unwrap();
    assert_eq!(result.choices.len(), 1);
    assert_eq!(result.conflicts[0].code, "installer_unsupported");
    assert_eq!(store.plans.len(), 1);
    let token = &result.choices[0].plan_id;
    assert_eq!(store.claim(token).unwrap().plan.version_id, "v1");
}

#[test]
fn newer_request_cancels_old_and_rejects_late_finish() {
    let mut store = PreviewStore::default();
    let old = store.begin("old".into()).unwrap();
    let new = store.begin("new".into()).unwrap();
    assert!(*old.borrow());
    assert!(!*new.borrow());
    assert!(store.finish("old").is_err());
    assert!(store.finish("new").is_ok());
}

#[test]
fn cancelling_old_request_cannot_cancel_replacement() {
    let mut store = PreviewStore::default();
    let _old = store.begin("old".into()).unwrap();
    let new = store.begin("new".into()).unwrap();
    store.cancel("old");
    assert!(!*new.borrow());
    store.cancel("new");
    assert!(*new.borrow());
    assert!(store.finish("new").is_err());
}

// SPDX-License-Identifier: MIT
//! Local first-run progress. Reads never create profiles or start network services.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};

const CONSENT_VERSION: &str = "2026-10-24";
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OnboardingState {
    schema_version: u32,
    step: String,
    consent_version: Option<String>,
    consent_accepted_at: Option<u64>,
    deferred: bool,
    completed: bool,
    mesh_choice: Option<bool>,
    #[serde(default)]
    existing_install: bool,
}
fn directory() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("ONBOARDING_STORAGE_UNAVAILABLE")?
        .join("CobbleLauncher"))
}
fn read_json(path: &Path) -> Option<serde_json::Value> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 8 * 1024 * 1024 {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return None;
        }
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}
fn existing_at(root: &Path) -> bool {
    read_json(&root.join("games.json"))
        .and_then(|v| {
            v.as_array().map(|a| {
                a.iter()
                    .any(|g| g["id"].as_str().is_some_and(|s| !s.is_empty()))
            })
        })
        .unwrap_or(false)
        || read_json(&root.join("player-profiles.json"))
            .and_then(|v| serde_json::from_value::<cobble_auth::ProfileLibrary>(v).ok())
            .is_some_and(|library| {
                library
                    .profiles
                    .iter()
                    .any(|p| !p.id.is_empty() && !p.name.is_empty() && !p.uuid.is_empty())
            })
        || read_json(&root.join("offline-profile.json"))
            .and_then(|v| serde_json::from_value::<cobble_auth::OfflineProfile>(v).ok())
            .is_some_and(|p| !p.name.is_empty() && !p.uuid.is_empty())
}
fn valid_step(step: &str) -> bool {
    matches!(step, "welcome" | "consent" | "profile" | "game" | "mesh")
}
fn consent_accepted(state: &OnboardingState) -> bool {
    state.consent_version.as_deref() == Some(CONSENT_VERSION) && state.consent_accepted_at.is_some()
}
fn read_at(root: &Path) -> Result<OnboardingState, String> {
    let existing = existing_at(root);
    let path = root.join("onboarding.json");
    if !path.exists() {
        return Ok(OnboardingState {
            schema_version: 1,
            step: "consent".into(),
            consent_version: None,
            consent_accepted_at: None,
            deferred: false,
            completed: false,
            mesh_choice: None,
            existing_install: existing,
        });
    }
    let mut state: OnboardingState =
        serde_json::from_value(read_json(&path).ok_or("ONBOARDING_STATE_INVALID")?)
            .map_err(|_| "ONBOARDING_STATE_INVALID")?;
    if state.schema_version != 1
        || !valid_step(&state.step)
        || state.consent_version.is_some() != state.consent_accepted_at.is_some()
    {
        return Err("ONBOARDING_STATE_INVALID".into());
    }
    if !consent_accepted(&state) {
        // Old deferred/completed tutorial progress isn't acceptance of terms.
        // Normalize the view without writing during reads or losing its receipt.
        state.step = "consent".into();
        state.deferred = false;
        state.completed = false;
    }
    state.existing_install = existing;
    Ok(state)
}
fn transition(
    state: &mut OnboardingState,
    action: &str,
    step: Option<String>,
    mesh_choice: Option<bool>,
) -> Result<(), String> {
    match action {
        "acceptConsent" => {
            if !consent_accepted(state) {
                state.consent_version = Some(CONSENT_VERSION.into());
                state.consent_accepted_at = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|_| "ONBOARDING_CLOCK_INVALID")?
                        .as_secs(),
                );
            }
            state.step = "welcome".into();
            state.deferred = false;
            state.completed = false;
        }
        "step" => {
            let next = step.ok_or("ONBOARDING_STEP_INVALID")?;
            if !valid_step(&next) {
                return Err("ONBOARDING_STEP_INVALID".into());
            }
            if next != "consent" && !consent_accepted(state) {
                return Err("ONBOARDING_CONSENT_REQUIRED".into());
            }
            state.step = next;
        }
        "defer" => {
            if !consent_accepted(state) {
                return Err("ONBOARDING_CONSENT_REQUIRED".into());
            }
            state.deferred = true;
        }
        "resume" => {
            state.deferred = false;
            state.completed = false;
        }
        "complete" => {
            if !consent_accepted(state) {
                return Err("ONBOARDING_CONSENT_REQUIRED".into());
            }
            state.completed = true;
            state.deferred = false;
        }
        "meshChoice" => {
            if !consent_accepted(state) {
                return Err("ONBOARDING_CONSENT_REQUIRED".into());
            }
            state.mesh_choice = Some(mesh_choice.ok_or("ONBOARDING_MESH_CHOICE_REQUIRED")?)
        }
        _ => return Err("ONBOARDING_ACTION_INVALID".into()),
    }
    Ok(())
}
fn persist(root: &Path, state: &OnboardingState) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    let meta = std::fs::symlink_metadata(root).map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("ONBOARDING_STORAGE_UNAVAILABLE".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("ONBOARDING_STORAGE_UNAVAILABLE".into());
        }
    }
    let mut file =
        tempfile::NamedTempFile::new_in(root).map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    file.write_all(&serde_json::to_vec(state).map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?)
        .map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    file.as_file()
        .sync_all()
        .map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    file.persist(root.join("onboarding.json"))
        .map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    Ok(())
}
#[tauri::command]
pub fn get_onboarding_state() -> Result<OnboardingState, String> {
    read_at(&directory()?)
}
#[tauri::command]
pub fn save_onboarding_state(
    action: String,
    step: Option<String>,
    mesh_choice: Option<bool>,
) -> Result<OnboardingState, String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().map_err(|_| "ONBOARDING_STORAGE_UNAVAILABLE")?;
    let root = directory()?;
    let mut state = read_at(&root)?;
    transition(&mut state, &action, step, mesh_choice)?;
    persist(&root, &state)?;
    Ok(state)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_consent_precedes_optional_tutorial_and_cannot_be_deferred() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = read_at(dir.path()).unwrap();
        assert!(!state.completed);
        assert_eq!(state.step, "consent");
        assert!(state.mesh_choice.is_none());
        assert_eq!(
            transition(&mut state, "defer", None, None).unwrap_err(),
            "ONBOARDING_CONSENT_REQUIRED"
        );
        assert!(transition(&mut state, "step", Some("welcome".into()), None).is_err());
        assert!(transition(&mut state, "meshChoice", None, Some(false)).is_err());
        persist(dir.path(), &state).unwrap();
        let mut state = read_at(dir.path()).unwrap();
        assert!(!state.deferred);
        assert!(state.consent_version.is_none());
        assert!(transition(&mut state, "complete", None, None).is_err());
        transition(&mut state, "resume", None, None).unwrap();
        assert!(!state.deferred);
        assert!(state.consent_accepted_at.is_none());
        transition(&mut state, "acceptConsent", None, None).unwrap();
        assert_eq!(state.step, "welcome");
        transition(&mut state, "defer", None, None).unwrap();
        assert!(state.deferred);
        transition(&mut state, "resume", None, None).unwrap();
        transition(&mut state, "complete", None, None).unwrap();
        assert!(state.completed);
    }
    #[test]
    fn existing_data_is_preserved_but_does_not_replace_required_consent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("games.json"), b"[]").unwrap();
        assert!(!read_at(dir.path()).unwrap().completed);
        std::fs::write(dir.path().join("games.json"), br#"[{"id":"saved-game"}]"#).unwrap();
        let state = read_at(dir.path()).unwrap();
        assert!(!state.completed);
        assert_eq!(state.step, "consent");
        assert!(state.existing_install);
        assert!(state.consent_version.is_none());
    }
    #[test]
    fn old_unaccepted_completed_or_deferred_progress_returns_to_consent_without_writes() {
        for (version, accepted_at) in [(None, None), (Some("older-terms"), Some(7))] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("onboarding.json");
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schemaVersion":1,"step":"game","consentVersion":version,
                "consentAcceptedAt":accepted_at,"deferred":true,"completed":true,"meshChoice":false
            }))
            .unwrap();
            std::fs::write(&path, &bytes).unwrap();
            let mut state = read_at(dir.path()).unwrap();
            assert_eq!(state.step, "consent");
            assert!(!state.deferred && !state.completed);
            assert_eq!(state.mesh_choice, Some(false));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert!(transition(&mut state, "defer", None, None).is_err());
            assert!(transition(&mut state, "complete", None, None).is_err());
            transition(&mut state, "resume", None, None).unwrap();
            assert_eq!(state.step, "consent");
            transition(&mut state, "acceptConsent", None, None).unwrap();
            assert_eq!(state.step, "welcome");
            assert!(consent_accepted(&state));
        }
    }
    #[test]
    fn current_acceptance_from_older_records_keeps_progress_and_can_be_reviewed() {
        for (deferred, completed) in [(false, true), (true, false)] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("onboarding.json");
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schemaVersion":1,"step":"game","consentVersion":CONSENT_VERSION,
                "consentAcceptedAt":7,"deferred":deferred,"completed":completed,"meshChoice":false
            }))
            .unwrap();
            std::fs::write(&path, &bytes).unwrap();
            let mut state = read_at(dir.path()).unwrap();
            assert_eq!(state.step, "game");
            assert_eq!((state.deferred, state.completed), (deferred, completed));
            assert_eq!(state.consent_accepted_at, Some(7));
            assert_eq!(std::fs::read(path).unwrap(), bytes);
            transition(&mut state, "step", Some("consent".into()), None).unwrap();
            transition(&mut state, "acceptConsent", None, None).unwrap();
            assert_eq!(state.step, "welcome");
            assert_eq!(state.consent_accepted_at, Some(7));
        }
    }
    #[test]
    fn invalid_state_does_not_silently_reset_consent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("onboarding.json"), b"broken").unwrap();
        assert_eq!(read_at(dir.path()).unwrap_err(), "ONBOARDING_STATE_INVALID");
    }
    #[test]
    fn optional_steps_resume_with_explicit_mesh_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = read_at(dir.path()).unwrap();
        assert!(transition(&mut state, "step", Some("game".into()), None).is_err());
        transition(&mut state, "acceptConsent", None, None).unwrap();
        transition(&mut state, "step", Some("game".into()), None).unwrap();
        transition(&mut state, "meshChoice", None, Some(false)).unwrap();
        transition(&mut state, "defer", None, None).unwrap();
        persist(dir.path(), &state).unwrap();
        let mut state = read_at(dir.path()).unwrap();
        transition(&mut state, "resume", None, None).unwrap();
        assert_eq!(state.step, "game");
        assert_eq!(state.mesh_choice, Some(false));
        assert_eq!(state.consent_version.as_deref(), Some(CONSENT_VERSION));
    }
}

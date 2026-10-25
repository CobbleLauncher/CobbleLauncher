// SPDX-License-Identifier: MIT
//! Read-only resolution previews become durable install tasks only after confirmation.
use super::*;
use cobble_content::{
    ReverseConfiguration, ReverseConfigurationChoice, ReverseConfigurationConflict,
    ReverseConfigurationStats,
};
use std::{
    collections::{HashSet, VecDeque},
    sync::OnceLock,
};
use tokio::sync::watch;

const PLAN_LIFETIME: Duration = Duration::from_secs(30 * 60);

#[derive(Default)]
struct PreviewStore {
    active: Option<(String, watch::Sender<bool>)>,
    plans: HashMap<String, (Instant, ReverseConfigurationChoice)>,
    creating: HashSet<String>,
    cancelled_requests: VecDeque<String>,
}

impl PreviewStore {
    fn begin(&mut self, request_id: String) -> Result<watch::Receiver<bool>, String> {
        if self.cancelled_requests.contains(&request_id) {
            return Err("REVERSE_CONFIGURATION_CANCELLED".into());
        }
        if let Some((_, previous)) = self.active.take() {
            let _ = previous.send(true);
        }
        self.plans.clear();
        let (sender, receiver) = watch::channel(false);
        self.active = Some((request_id, sender));
        Ok(receiver)
    }

    fn cancel(&mut self, request_id: &str) {
        // IPC cancellation can arrive before the async command is first polled.
        if !self.cancelled_requests.iter().any(|id| id == request_id) {
            self.cancelled_requests.push_back(request_id.into());
            if self.cancelled_requests.len() > 32 {
                self.cancelled_requests.pop_front();
            }
        }
        if self.active.as_ref().is_some_and(|(id, _)| id == request_id)
            && let Some((_, sender)) = self.active.take()
        {
            let _ = sender.send(true);
        }
    }

    fn finish(&mut self, request_id: &str) -> Result<(), String> {
        if self
            .active
            .as_ref()
            .is_none_or(|(id, sender)| id != request_id || *sender.borrow())
        {
            return Err("REVERSE_CONFIGURATION_CANCELLED".into());
        }
        self.active = None;
        Ok(())
    }

    fn claim(&mut self, plan_id: &str) -> Result<ReverseConfigurationChoice, String> {
        let (created, choice) = self.plans.get(plan_id).ok_or("REVERSE_PLAN_EXPIRED")?;
        if created.elapsed() > PLAN_LIFETIME {
            return Err("REVERSE_PLAN_EXPIRED".into());
        }
        if !self.creating.insert(plan_id.to_owned()) {
            return Err("REVERSE_PLAN_BUSY".into());
        }
        Ok(choice.clone())
    }

    fn release(&mut self, plan_id: &str, created: bool) {
        self.creating.remove(plan_id);
        if created {
            self.plans.remove(plan_id);
        }
    }
}

fn previews() -> &'static Mutex<PreviewStore> {
    static STORE: OnceLock<Mutex<PreviewStore>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}

#[derive(Serialize)]
pub struct ReverseChoice {
    plan_id: String,
    #[serde(flatten)]
    choice: ReverseConfigurationChoice,
}

#[derive(Serialize)]
pub struct ReversePreview {
    choices: Vec<ReverseChoice>,
    conflicts: Vec<ReverseConfigurationConflict>,
    stats: ReverseConfigurationStats,
    limitations: Vec<String>,
}

fn supported_target(choice: &ReverseConfigurationChoice, loader_version: Option<&str>) -> bool {
    cobble_content::query_compatibility(
        &choice.game_version,
        Some(&choice.loader),
        loader_version,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
    .allowed()
}

fn publish(
    store: &mut PreviewStore,
    request_id: &str,
    configuration: ReverseConfiguration,
) -> Result<ReversePreview, String> {
    store.finish(request_id)?;
    let mut conflicts = configuration.conflicts;
    let mut choices = Vec::new();
    for choice in configuration.choices {
        if !supported_target(&choice, None) {
            conflicts.push(ReverseConfigurationConflict {
                game_version: choice.game_version,
                loader: choice.loader,
                code: "installer_unsupported".into(),
                message: "This launcher cannot install this target yet".into(),
                projects: choice
                    .roots
                    .into_iter()
                    .map(|root| root.project_id)
                    .collect(),
            });
            continue;
        }
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
        let plan_id: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
        store
            .plans
            .insert(plan_id.clone(), (Instant::now(), choice.clone()));
        choices.push(ReverseChoice { plan_id, choice });
    }
    Ok(ReversePreview {
        choices,
        conflicts,
        stats: configuration.stats,
        limitations: configuration.limitations,
    })
}

#[tauri::command]
pub async fn search_reverse_mods(
    query: String,
    offset: u32,
) -> Result<cobble_content::SearchResults, String> {
    cobble_content::search_modrinth_unfiltered(&query, offset)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reverse_configure_mods(
    request_id: String,
    project_ids: Vec<String>,
    preferred_game_versions: Vec<String>,
    loaders: Vec<String>,
) -> Result<ReversePreview, String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("REVERSE_REQUEST_INVALID".into());
    }
    let mut cancelled = previews()
        .lock()
        .map_err(|_| "REVERSE_STATE_UNAVAILABLE")?
        .begin(request_id.clone())?;
    let resolution = tokio::select! {
        result = cobble_content::reverse_configure_modrinth(&project_ids, &preferred_game_versions, &loaders) => result.map_err(|error| error.to_string()),
        _ = cancelled.changed() => Err("REVERSE_CONFIGURATION_CANCELLED".into()),
    };
    let mut store = previews().lock().map_err(|_| "REVERSE_STATE_UNAVAILABLE")?;
    match resolution {
        Ok(configuration) => {
            tracing::info!(
                event = "reverse_configuration",
                targets = configuration.stats.targets_considered,
                requests = configuration.stats.metadata_requests,
                candidates = configuration.stats.candidate_versions,
                metadata_ms = configuration.stats.metadata_elapsed_ms,
                solve_ms = configuration.stats.solve_elapsed_ms,
                choices = configuration.choices.len()
            );
            publish(&mut store, &request_id, configuration)
        }
        Err(error) => {
            store.cancel(&request_id);
            Err(error)
        }
    }
}

#[tauri::command]
pub fn cancel_reverse_configuration(request_id: String) -> Result<(), String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("REVERSE_REQUEST_INVALID".into());
    }
    previews()
        .lock()
        .map_err(|_| "REVERSE_STATE_UNAVAILABLE")?
        .cancel(&request_id);
    Ok(())
}

async fn choose_loader(
    choice: &ReverseConfigurationChoice,
    source: DownloadSettings,
) -> Result<String, String> {
    if !supported_target(choice, None) {
        return Err("REVERSE_TARGET_UNSUPPORTED".into());
    }
    let candidates = if choice.loader == "fabric" {
        list_fabric_versions(choice.game_version.clone(), source)
            .await?
            .into_iter()
            .map(|version| (version.version, version.stable))
            .collect::<Vec<_>>()
    } else {
        list_loader_versions(choice.game_version.clone(), choice.loader.clone(), source)
            .await?
            .into_iter()
            .map(|version| (version.version, version.recommended))
            .collect()
    };
    let allowed = candidates
        .iter()
        .filter(|(version, _)| supported_target(choice, Some(version)));
    allowed
        .clone()
        .find(|(_, recommended)| *recommended)
        .or_else(|| allowed.into_iter().next())
        .map(|(version, _)| version.clone())
        .ok_or_else(|| "REVERSE_LOADER_UNAVAILABLE".into())
}

#[tauri::command]
pub async fn create_game_from_mods(
    app: AppHandle,
    plan_id: String,
    name: String,
    automatic_name: bool,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    let choice = previews()
        .lock()
        .map_err(|_| "REVERSE_STATE_UNAVAILABLE")?
        .claim(&plan_id)?;
    let result = async {
        let loader_version = choose_loader(&choice, source.clone()).await?;
        let name = if automatic_name {
            format!(
                "{} {} {}",
                choice.game_version,
                match choice.loader.as_str() {
                    "fabric" => "Fabric",
                    "forge" => "Forge",
                    "neoforge" => "NeoForge",
                    _ => &choice.loader,
                },
                loader_version
            )
        } else {
            name
        };
        super::create_with_mod_plan(
            app,
            name,
            choice.game_version,
            source,
            use_default_directory,
            loader_version,
            choice.loader,
            automatic_name,
            choice.plan,
        )
    }
    .await;
    previews()
        .lock()
        .map_err(|_| "REVERSE_STATE_UNAVAILABLE")?
        .release(&plan_id, result.is_ok());
    result
}

#[cfg(test)]
#[path = "reverse_config/tests.rs"]
mod tests;

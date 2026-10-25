// SPDX-License-Identifier: MIT
//! Reviewed, source-qualified plans stay native until registered as durable tasks.
use super::*;
use cobble_content::{
    EnvironmentPlan, EnvironmentRoot, EnvironmentSource, ReverseEnvironmentChoice,
};
use std::{
    collections::{HashSet, VecDeque},
    sync::OnceLock,
};
use tauri::WebviewWindow;
use tokio::sync::watch;

#[derive(Default)]
struct Store {
    active: Option<(String, watch::Sender<bool>)>,
    cancelled: VecDeque<String>,
    choices: HashMap<String, (Instant, ReverseEnvironmentChoice)>,
    creating: HashSet<String>,
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Mutex::default)
}
fn choice(id: &str) -> Result<ReverseEnvironmentChoice, String> {
    let store = store()
        .lock()
        .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
    let (at, choice) = store.choices.get(id).ok_or("ENVIRONMENT_PLAN_EXPIRED")?;
    if at.elapsed() > Duration::from_secs(1800) {
        return Err("ENVIRONMENT_PLAN_EXPIRED".into());
    }
    Ok(choice.clone())
}

#[derive(Serialize)]
pub struct SearchHit {
    pub source: EnvironmentSource,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    pub preview_url: Option<String>,
}
#[derive(Serialize)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub offset: u32,
    pub total_hits: u64,
}
#[tauri::command]
pub async fn search_environment_mods(
    query: String,
    source: EnvironmentSource,
    offset: u32,
) -> Result<SearchPage, String> {
    match source {
        EnvironmentSource::Modrinth => {
            let page = cobble_content::search_modrinth_unfiltered(&query, offset)
                .await
                .map_err(|e| e.to_string())?;
            Ok(SearchPage {
                offset: page.offset,
                total_hits: u64::from(page.total_hits),
                hits: page
                    .hits
                    .into_iter()
                    .map(|h| SearchHit {
                        source,
                        project_id: h.project_id,
                        title: h.title,
                        description: h.description,
                        author: h.author,
                        downloads: h.downloads,
                        icon_url: h.icon_url,
                        preview_url: h.preview_url,
                    })
                    .collect(),
            })
        }
        EnvironmentSource::Curseforge => {
            let page = crate::curseforge::client()?
                .search(
                    &query,
                    cobble_content::curseforge::ContentKind::Mod,
                    None,
                    None,
                    offset,
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(SearchPage {
                offset: page.offset,
                total_hits: page.total_hits,
                hits: page
                    .hits
                    .into_iter()
                    .map(|h| SearchHit {
                        source,
                        project_id: h.project_id,
                        title: h.title,
                        description: h.description,
                        author: h.author,
                        downloads: h.downloads,
                        icon_url: h.icon_url,
                        preview_url: h.preview_url,
                    })
                    .collect(),
            })
        }
    }
}

#[derive(Serialize)]
pub struct Choice {
    pub plan_id: String,
    #[serde(flatten)]
    pub choice: ReverseEnvironmentChoice,
}
#[derive(Serialize)]
pub struct Configuration {
    pub choices: Vec<Choice>,
    pub conflicts: Vec<cobble_content::EnvironmentConflict>,
    pub stats: cobble_content::ReverseConfigurationStats,
    pub limitations: Vec<String>,
}
#[tauri::command]
pub async fn reverse_configure_environment(
    request_id: String,
    roots: Vec<EnvironmentRoot>,
    preferred_game_versions: Vec<String>,
    loaders: Vec<String>,
) -> Result<Configuration, String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("ENVIRONMENT_REQUEST_INVALID".into());
    }
    let mut cancelled = {
        let mut state = store()
            .lock()
            .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
        if state.cancelled.contains(&request_id) {
            return Err("ENVIRONMENT_CONFIGURATION_CANCELLED".into());
        }
        if let Some((_, old)) = state.active.take() {
            let _ = old.send(true);
        }
        state.choices.clear();
        let (sender, receiver) = watch::channel(false);
        state.active = Some((request_id.clone(), sender));
        receiver
    };
    let client = if roots
        .iter()
        .any(|r| r.source == EnvironmentSource::Curseforge)
    {
        Some(crate::curseforge::client()?)
    } else {
        None
    };
    let result = tokio::select! {
        value=cobble_content::reverse_configure_environment(&roots,&preferred_game_versions,&loaders,client.as_ref())=>value.map_err(|e|e.to_string()),
        _=cancelled.changed()=>Err("ENVIRONMENT_CONFIGURATION_CANCELLED".into()),
    };
    let mut state = store()
        .lock()
        .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
    if state
        .active
        .as_ref()
        .is_none_or(|(id, sender)| id != &request_id || *sender.borrow())
    {
        return Err("ENVIRONMENT_CONFIGURATION_CANCELLED".into());
    }
    state.active = None;
    let mut result = result?;
    tracing::info!(
        event = "environment_configuration",
        targets = result.stats.targets_considered,
        metadata_calls = result.stats.metadata_requests,
        candidates = result.stats.candidate_versions,
        metadata_and_plan_ms = result.stats.metadata_elapsed_ms,
        solve_ms = result.stats.solve_elapsed_ms,
        choices = result.choices.len(),
        limits = result.limitations.len()
    );
    let mut choices = Vec::new();
    for selected in result.choices {
        if !cobble_content::query_compatibility(
            &selected.game_version,
            Some(&selected.loader),
            None,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .allowed()
        {
            result.conflicts.push(cobble_content::EnvironmentConflict {
                game_version: selected.game_version,
                loader: selected.loader,
                code: "installer_unsupported".into(),
                message: "Installer target is unavailable".into(),
                projects: selected
                    .roots
                    .into_iter()
                    .map(|r| cobble_content::EnvironmentProject {
                        source: r.source,
                        project_id: r.project_id,
                    })
                    .collect(),
            });
            continue;
        }
        cobble_core::environment_install::validate_environment_plan(&selected.plan)
            .map_err(|e| e.to_string())?;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| "ENVIRONMENT_RANDOM_UNAVAILABLE")?;
        let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        state
            .choices
            .insert(id.clone(), (Instant::now(), selected.clone()));
        choices.push(Choice {
            plan_id: id,
            choice: selected,
        });
    }
    Ok(Configuration {
        choices,
        conflicts: result.conflicts,
        stats: result.stats,
        limitations: result.limitations,
    })
}
#[tauri::command]
pub fn cancel_environment_configuration(request_id: String) -> Result<(), String> {
    if request_id.is_empty() || request_id.len() > 128 {
        return Err("ENVIRONMENT_REQUEST_INVALID".into());
    }
    let mut state = store()
        .lock()
        .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
    if !state.cancelled.contains(&request_id) {
        state.cancelled.push_back(request_id.clone());
        if state.cancelled.len() > 32 {
            state.cancelled.pop_front();
        }
    }
    if state
        .active
        .as_ref()
        .is_some_and(|(id, _)| id == &request_id)
        && let Some((_, sender)) = state.active.take()
    {
        let _ = sender.send(true);
    }
    Ok(())
}

async fn loader(
    selected: &ReverseEnvironmentChoice,
    source: DownloadSettings,
) -> Result<String, String> {
    let candidates = if selected.loader == "fabric" {
        list_fabric_versions(selected.game_version.clone(), source)
            .await?
            .into_iter()
            .map(|v| (v.version, v.stable))
            .collect::<Vec<_>>()
    } else {
        list_loader_versions(
            selected.game_version.clone(),
            selected.loader.clone(),
            source,
        )
        .await?
        .into_iter()
        .map(|v| (v.version, v.recommended))
        .collect()
    };
    let allowed = candidates.iter().filter(|(v, _)| {
        cobble_content::query_compatibility(
            &selected.game_version,
            Some(&selected.loader),
            Some(v),
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .allowed()
    });
    allowed
        .clone()
        .find(|(_, recommended)| *recommended)
        .or_else(|| allowed.into_iter().next())
        .map(|(v, _)| v.clone())
        .ok_or("ENVIRONMENT_LOADER_UNAVAILABLE".into())
}
#[derive(Serialize)]
pub struct InstallationPreview {
    pub loader_version: String,
    pub requirements: native_reverse_install::Requirements,
    pub space: cobble_core::space::SpaceEstimate,
}
#[tauri::command]
pub async fn preview_environment_installation(
    app: AppHandle,
    plan_id: String,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<InstallationPreview, String> {
    let selected = choice(&plan_id)?;
    let loader_version = loader(&selected, source.clone()).await?;
    let directory = native_readiness_space::creation_directory(&app, use_default_directory)?
        .join(".cobble-environment-preflight");
    let java = cobble_core::capture_java_choice(&cobble_minecraft::JavaOverride::Inherit)
        .map_err(|e| e.to_string())?;
    let space = native_readiness_space::estimate(
        &selected.game_version,
        Some(&selected.loader),
        Some(&selected.plan),
        &directory,
        source,
        &java,
        cobble_core::OperationControl::new(Arc::new(|_| {})),
    )
    .await?;
    let requirements = native_reverse_install::requirements(selected.plan).await?;
    choice(&plan_id)?;
    Ok(InstallationPreview {
        loader_version,
        requirements,
        space,
    })
}
#[tauri::command]
pub async fn select_environment_manual_file(
    window: WebviewWindow,
    plan_id: String,
    project_id: String,
    file_id: String,
) -> Result<Option<native_reverse_install::Requirements>, String> {
    let plan = choice(&plan_id)?.plan;
    if !native_reverse_install::select_manual(&window, plan.clone(), project_id, file_id).await? {
        return Ok(None);
    }
    choice(&plan_id)?;
    native_reverse_install::requirements(plan).await.map(Some)
}
#[tauri::command]
pub async fn create_game_from_environment(
    app: AppHandle,
    plan_id: String,
    name: String,
    automatic_name: bool,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    let selected = choice(&plan_id)?;
    {
        let mut state = store()
            .lock()
            .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
        if !state.creating.insert(plan_id.clone()) {
            return Err("ENVIRONMENT_PLAN_BUSY".into());
        }
    }
    let result = async {
        let preview = preview_environment_installation(
            app.clone(),
            plan_id.clone(),
            source.clone(),
            use_default_directory,
        )
        .await?;
        if !preview.requirements.ready {
            return Err("ENVIRONMENT_MANUAL_FILES_REQUIRED".into());
        }
        if !preview.space.can_proceed {
            return Err("SPACE_INSUFFICIENT".into());
        }
        let name = if automatic_name {
            format!(
                "{} {} {}",
                selected.game_version, selected.loader, preview.loader_version
            )
        } else {
            name
        };
        super::create_with_environment_plan(
            app,
            name,
            selected.game_version,
            source,
            use_default_directory,
            preview.loader_version,
            selected.loader,
            automatic_name,
            selected.plan,
        )
    }
    .await;
    let mut state = store()
        .lock()
        .map_err(|_| "ENVIRONMENT_STATE_UNAVAILABLE")?;
    state.creating.remove(&plan_id);
    if result.is_ok() {
        state.choices.remove(&plan_id);
    }
    result
}

pub(super) fn validate_target(game: &Game, plan: &EnvironmentPlan) -> Result<(), String> {
    if game.version != plan.game_version
        || game.loader_kind.as_deref() != Some(plan.loader.as_str())
    {
        return Err("ENVIRONMENT_GAME_CHANGED".into());
    }
    Ok(())
}

// SPDX-License-Identifier: MIT
//! User-reviewed invitation setup, prepared in a new registered game only.
use super::*;
use cobble_content::{EnvironmentPlan, EnvironmentSelection};
use std::collections::{HashMap, HashSet};
#[path = "mesh_environment_document.rs"]
mod document;
#[path = "mesh_environment_local.rs"]
mod local;
#[path = "mesh_environment_transport.rs"]
mod transport;
#[derive(Clone)]
struct Share {
    game_id: String,
    document: document::Document,
    configs: Vec<cobble_core::play_manifest::ConfigChoice>,
    revision: String,
    at: Instant,
}
#[derive(Clone)]
struct Received {
    document: document::Document,
    host: String,
    digest: String,
    at: Instant,
    compare_game_id: Option<String>,
}
#[derive(Clone)]
struct Plan {
    received: Received,
    plan: Option<EnvironmentPlan>,
    at: Instant,
    personal: local::LocalOptions,
}
#[derive(Default)]
struct Store {
    shares: HashMap<String, Share>,
    received: HashMap<String, Received>,
    plans: HashMap<String, Plan>,
    creating: HashSet<String>,
}
fn state() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}
fn nonce() -> Result<String, String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
fn expire(store: &mut Store) {
    store
        .shares
        .retain(|_, value| value.at.elapsed() < Duration::from_secs(300));
    store
        .received
        .retain(|_, value| value.at.elapsed() < Duration::from_secs(1800));
    store
        .plans
        .retain(|_, value| value.at.elapsed() < Duration::from_secs(1800));
}
fn stored(id: &str) -> Result<Plan, String> {
    let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    expire(&mut state);
    state
        .plans
        .get(id)
        .cloned()
        .ok_or_else(|| "MESH_ENVIRONMENT_PREVIEW_EXPIRED".into())
}
#[tauri::command]
pub async fn mesh_environment_share_preview(
    app: AppHandle,
    game_id: String,
) -> Result<Value, String> {
    let (document, configs, revision) = document::capture(&app, &game_id).await?;
    let id = nonce()?;
    let result = json!({"preview_id":id,"document":document,"configs":configs});
    let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    expire(&mut state);
    if state.shares.len() >= 8 {
        return Err("MESH_ENVIRONMENT_BUSY".into());
    }
    state.shares.insert(
        id,
        Share {
            game_id,
            document,
            configs,
            revision,
            at: Instant::now(),
        },
    );
    Ok(result)
}
#[tauri::command]
pub async fn mesh_environment_share_publish(
    app: AppHandle,
    preview_id: String,
    config_ids: Vec<String>,
) -> Result<Value, String> {
    let draft = {
        let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
        expire(&mut state);
        state
            .shares
            .get(&preview_id)
            .cloned()
            .ok_or("MESH_ENVIRONMENT_PREVIEW_EXPIRED")?
    };
    let (_, _, revision) = document::capture(&app, &draft.game_id).await?;
    if revision != draft.revision {
        return Err("MESH_ENVIRONMENT_SOURCE_CHANGED".into());
    }
    if config_ids.len() > 64 || config_ids.iter().collect::<HashSet<_>>().len() != config_ids.len()
    {
        return Err("MESH_ENVIRONMENT_CONFIG_INVALID".into());
    }
    let mut document = draft.document;
    for id in config_ids {
        let config = draft
            .configs
            .iter()
            .find(|config| config.id == id)
            .ok_or("MESH_ENVIRONMENT_CONFIG_INVALID")?;
        document.configs.push(document::Config {
            path: config.name.clone(),
            text: config.text.clone(),
            sha256: cobble_plugin_host::digest(config.text.as_bytes()),
        });
    }
    document::validate(&document)?;
    let bytes = serde_json::to_vec(&document).map_err(|_| "MESH_ENVIRONMENT_DOCUMENT_INVALID")?;
    let game_id = draft.game_id;
    let invitation = tauri::async_runtime::spawn_blocking(move || {
        let (client, group_id) = groups::environment_client(&game_id)?;
        transport::upload(&client, group_id, &bytes)
    })
    .await
    .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")??;
    state()
        .lock()
        .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?
        .shares
        .remove(&preview_id);
    Ok(json!({"invitation":invitation}))
}
#[tauri::command]
pub async fn mesh_environment_receive(
    app: AppHandle,
    invitation: String,
    compare_game_id: Option<String>,
) -> Result<Value, String> {
    if invitation.is_empty() || invitation.len() > 6000 || invitation.chars().any(char::is_control)
    {
        return Err("MESH_ENVIRONMENT_INVITATION_INVALID".into());
    }
    let (document, host, digest) =
        tauri::async_runtime::spawn_blocking(move || transport::receive(invitation))
            .await
            .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")??;
    let comparison = if let Some(game_id) = compare_game_id.as_ref() {
        match document::capture(&app, game_id).await {
            Ok((local, configs, _)) => {
                json!({"available":true,"game_id":game_id,"version_matches":local.game_version==document.game_version,"loader_matches":local.loader==document.loader&&local.loader_version==document.loader_version,"matching_mods":document.mods.iter().filter(|item|local.mods.iter().any(|other|other.source==item.source&&other.project_id==item.project_id&&other.version_id==item.version_id&&other.sha1==item.sha1)).count(),"required_mods":document.mods.len(),"additional_mods":local.mods.iter().filter(|item|!document.mods.iter().any(|other|item.source==other.source&&item.project_id==other.project_id&&item.version_id==other.version_id&&item.sha1==other.sha1)).count(),"matching_configs":document.configs.iter().filter(|item|configs.iter().any(|other|item.path==other.name&&item.sha256==cobble_plugin_host::digest(other.text.as_bytes()))).count(),"required_configs":document.configs.len(),"local_excluded":local.excluded})
            }
            Err(_) => json!({"available":false,"game_id":game_id}),
        }
    } else {
        Value::Null
    };
    let id = nonce()?;
    let result = json!({"preview_id":id,"host_peer_id":host,"sha256":digest,"document":document,"comparison":comparison});
    let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    expire(&mut state);
    if state.received.len() >= 8 {
        return Err("MESH_ENVIRONMENT_BUSY".into());
    }
    state.received.insert(
        id,
        Received {
            document,
            host,
            digest,
            at: Instant::now(),
            compare_game_id,
        },
    );
    Ok(result)
}
async fn check_loader(
    document: &document::Document,
    source: DownloadSettings,
) -> Result<(), String> {
    if !cobble_content::query_compatibility(
        &document.game_version,
        document.loader.as_deref(),
        document.loader_version.as_deref(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
    .allowed()
    {
        return Err("MESH_ENVIRONMENT_TARGET_UNSUPPORTED".into());
    }
    if let (Some(loader), Some(version)) = (&document.loader, &document.loader_version) {
        let available = if loader == "fabric" {
            crate::games::list_fabric_versions(document.game_version.clone(), source)
                .await?
                .iter()
                .any(|item| &item.version == version)
        } else {
            crate::games::list_loader_versions(
                document.game_version.clone(),
                loader.clone(),
                source,
            )
            .await?
            .iter()
            .any(|item| &item.version == version)
        };
        if !available {
            return Err("MESH_ENVIRONMENT_LOADER_UNAVAILABLE".into());
        }
    }
    Ok(())
}
async fn preflight(
    app: &AppHandle,
    selected: &Plan,
    source: DownloadSettings,
    use_default: bool,
) -> Result<Value, String> {
    let document = &selected.received.document;
    check_loader(document, source.clone()).await?;
    if let Some(plan) = &selected.plan {
        document::verify_plan(document, plan)?;
    }
    let directory = crate::games::native_readiness_space::creation_directory(app, use_default)?
        .join(".cobble-mesh-environment-preflight");
    let java = cobble_core::capture_java_choice(&cobble_minecraft::JavaOverride::Inherit)
        .map_err(|_| "MESH_ENVIRONMENT_JAVA_UNAVAILABLE")?;
    let mut space = crate::games::native_readiness_space::estimate(
        &document.game_version,
        document.loader.as_deref(),
        selected.plan.as_ref(),
        &directory,
        source,
        &java,
        cobble_core::OperationControl::new(Arc::new(|_| {})),
    )
    .await?;
    let configs = document
        .configs
        .iter()
        .map(|item| cobble_core::environment_install::SharedConfig {
            path: item.path.clone(),
            text: item.text.clone(),
            sha256: item.sha256.clone(),
        })
        .collect::<Vec<_>>();
    crate::games::native_readiness_space::include_shared_configs(&mut space, &directory, &configs)?;
    crate::games::native_readiness_space::include_personal_options(
        &mut space,
        &directory,
        selected.personal.options.as_ref(),
    )?;
    let requirements = if let Some(plan) = &selected.plan {
        serde_json::to_value(
            crate::games::native_reverse_install::requirements(plan.clone()).await?,
        )
        .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?
    } else {
        json!({"ready":true,"missing":[]})
    };
    Ok(json!({"space":space,"requirements":requirements}))
}
#[tauri::command]
pub async fn mesh_environment_prepare(
    app: AppHandle,
    preview_id: String,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Value, String> {
    let received = {
        let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
        expire(&mut state);
        state
            .received
            .get(&preview_id)
            .cloned()
            .ok_or("MESH_ENVIRONMENT_PREVIEW_EXPIRED")?
    };
    if !received.document.excluded.is_empty() {
        return Err("MESH_ENVIRONMENT_EXCLUDED_MODS".into());
    }
    let plan = if received.document.mods.is_empty() {
        None
    } else {
        let roots = received
            .document
            .mods
            .iter()
            .map(|item| EnvironmentSelection {
                source: item.source,
                project_id: item.project_id.clone(),
                version_id: item.version_id.clone(),
            })
            .collect::<Vec<_>>();
        let cf = if roots
            .iter()
            .any(|root| root.source == cobble_content::EnvironmentSource::Curseforge)
        {
            Some(crate::curseforge::client()?)
        } else {
            None
        };
        let plan = tokio::time::timeout(
            Duration::from_secs(60),
            cobble_content::resolve_environment_selected(
                &roots,
                &received.document.game_version,
                received
                    .document
                    .loader
                    .as_deref()
                    .ok_or("MESH_ENVIRONMENT_TARGET_UNSUPPORTED")?,
                cf.as_ref(),
            ),
        )
        .await
        .map_err(|_| "MESH_ENVIRONMENT_RESOLUTION_TIMEOUT")?
        .map_err(|_| "MESH_ENVIRONMENT_RESOLUTION_FAILED")?;
        document::verify_plan(&received.document, &plan)?;
        Some(plan)
    };
    let personal = local::capture(
        &app,
        received.compare_game_id.as_deref(),
        &received.document.game_version,
    )
    .await;
    let selected = Plan {
        received,
        plan,
        at: Instant::now(),
        personal,
    };
    let preflight = preflight(&app, &selected, source, use_default_directory).await?;
    let id = nonce()?;
    let result = json!({"plan_id":id,"preflight":preflight,"host_peer_id":selected.received.host,"sha256":selected.received.digest,"personal":selected.personal.summary});
    let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    expire(&mut state);
    if state.plans.len() >= 8 {
        return Err("MESH_ENVIRONMENT_BUSY".into());
    }
    state.plans.insert(id, selected);
    Ok(result)
}
#[tauri::command]
pub async fn mesh_environment_manual_file(
    window: WebviewWindow,
    plan_id: String,
    project_id: String,
    file_id: String,
) -> Result<Value, String> {
    let selected = stored(&plan_id)?;
    let plan = selected
        .plan
        .ok_or("MESH_ENVIRONMENT_MANUAL_FILE_INVALID")?;
    if !crate::games::native_reverse_install::select_manual(
        &window,
        plan.clone(),
        project_id,
        file_id,
    )
    .await?
    {
        return Ok(Value::Null);
    }
    stored(&plan_id)?;
    serde_json::to_value(crate::games::native_reverse_install::requirements(plan).await?)
        .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE".into())
}
#[tauri::command]
pub async fn mesh_environment_create(
    app: AppHandle,
    plan_id: String,
    name: String,
    source: DownloadSettings,
    use_default_directory: bool,
) -> Result<Game, String> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("MESH_ENVIRONMENT_NAME_INVALID".into());
    }
    let selected = stored(&plan_id)?;
    if !state()
        .lock()
        .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?
        .creating
        .insert(plan_id.clone())
    {
        return Err("MESH_ENVIRONMENT_BUSY".into());
    }
    let result = async {
        let current = local::capture(
            &app,
            selected.personal.source_game_id.as_deref(),
            &selected.received.document.game_version,
        )
        .await;
        if current.revision != selected.personal.revision {
            return Err("MESH_ENVIRONMENT_LOCAL_OPTIONS_CHANGED".into());
        }
        let checks = preflight(&app, &selected, source.clone(), use_default_directory).await?;
        if checks["space"]["can_proceed"] != true {
            return Err("SPACE_INSUFFICIENT".into());
        }
        if checks["requirements"]["ready"] != true {
            return Err("ENVIRONMENT_MANUAL_FILES_REQUIRED".into());
        }
        stored(&plan_id)?;
        let document = selected.received.document;
        let configs = document
            .configs
            .into_iter()
            .map(|item| cobble_core::environment_install::SharedConfig {
                path: item.path,
                text: item.text,
                sha256: item.sha256,
            })
            .collect();
        if let Some(plan) = selected.plan {
            crate::games::create_with_environment_plan_and_configs(
                app,
                name.trim().into(),
                document.game_version,
                source,
                use_default_directory,
                document
                    .loader_version
                    .ok_or("MESH_ENVIRONMENT_LOADER_UNAVAILABLE")?,
                document
                    .loader
                    .ok_or("MESH_ENVIRONMENT_LOADER_UNAVAILABLE")?,
                false,
                plan,
                configs,
                selected.personal.options,
            )
        } else {
            crate::games::create_with_shared_setup(
                app,
                name.trim().into(),
                document.game_version,
                source,
                use_default_directory,
                document.loader_version,
                document.loader,
                configs,
                selected.personal.options,
            )
        }
    }
    .await;
    let mut state = state().lock().map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
    state.creating.remove(&plan_id);
    if result.is_ok() {
        state.plans.remove(&plan_id);
    }
    result
}

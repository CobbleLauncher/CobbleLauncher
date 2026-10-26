// SPDX-License-Identifier: MIT
//! Calls production native adapters with fresh isolated instances and canonical manifests.
use crate::{games, plugins};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Listener, WebviewWindow};
type Result<T> = std::result::Result<T, String>;
const GAME: &str = "a065f17e";
fn request<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(|e| format!("fixture request: {e}"))
}
fn require(value: bool, message: &str) -> Result<()> {
    if value { Ok(()) } else { Err(message.into()) }
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("missing result {key}"))
}
fn io(e: impl std::fmt::Display) -> String {
    format!("synthetic fixture IO: {e}")
}
fn auth(
    app: &AppHandle,
    id: &str,
    label: &str,
) -> Result<plugins::plugin_views::ViewAuthorization> {
    plugins::plugin_views::isolated_authorization(app, id, &format!("maintenance-probe-{label}"))
}
async fn trial_done(
    app: &AppHandle,
    auth: &plugins::plugin_views::ViewAuthorization,
    id: &str,
) -> Result<Value> {
    for _ in 0..300 {
        let value =
            games::trial_copy::task(app.clone(), auth, request(json!({"taskId":id}))?).await?;
        if value["status"] != "running" && value["status"] != "cancelling" {
            return Ok(value);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err("trial native task did not finish".into())
}
async fn storage_done(
    app: &AppHandle,
    auth: &plugins::plugin_views::ViewAuthorization,
    id: &str,
) -> Result<Value> {
    for _ in 0..300 {
        let value =
            games::storage_workspace::task(app.clone(), auth, request(json!({"taskId":id}))?)
                .await?;
        if value["status"] != "running" {
            return Ok(value);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err("migration native task did not finish".into())
}
async fn native_done(app: &AppHandle, id: &str) -> Result<games::tasks::TaskSnapshot> {
    for _ in 0..300 {
        let task = games::tasks::list_tasks(app.clone())?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or("native task missing")?;
        if !matches!(
            task.state,
            games::tasks::TaskState::Queued
                | games::tasks::TaskState::Running
                | games::tasks::TaskState::Cancelling
        ) {
            return Ok(task);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    Err("native task did not finish".into())
}
pub async fn run(app: &AppHandle, window: &WebviewWindow, base: &Path) -> Result<Value> {
    // Follow the production startup recovery path; loaded summaries start locked/verifying.
    games::sessions::watch_recovered(app);
    for _ in 0..100 {
        if games::registered_game_stopped(app, GAME)? {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    require(
        games::registered_game_stopped(app, GAME)?,
        "synthetic startup recovery did not establish a stopped game",
    )?;
    let source = base.join("fixture-game");
    let trial = auth(app, "trial-copy", "trial")?;
    let storage = auth(app, "storage-lens", "storage")?;
    let config = auth(app, "config-doctor", "config")?;
    let share = auth(app, "play-manifest", "share")?;
    let inbox = auth(app, "file-inbox", "inbox")?;
    let packages=[&trial,&storage,&config,&share,&inbox].iter().map(|auth|json!({"id":auth.manifest.id,"version":auth.manifest.version,"componentSha256":auth.manifest.component_sha256,"frontend":auth.manifest.frontend})).collect::<Vec<_>>();
    {
        use std::io::Write;
        let mut file =
            std::fs::File::create(source.join("cancellation-fixture.bin")).map_err(io)?;
        let chunk = [37u8; 128 * 1024];
        for _ in 0..512 {
            file.write_all(&chunk).map_err(io)?;
        }
        file.sync_all().map_err(io)?;
    }
    eprintln!("MAINTENANCE_STAGE trial-copy");
    let preview = games::trial_copy::isolated_preview(
        app.clone(),
        &trial,
        request(json!({"gameId":GAME,"name":"Synthetic trial","includeWorlds":false}))?,
        base.join("copies"),
    )
    .await?;
    let started = games::trial_copy::start(
        app.clone(),
        &trial,
        request(json!({"planId":string(&preview,"planId")?}))?,
    )
    .await?;
    let trial_id = string(&started, "taskId")?;
    require(
        games::tasks::active_count(app) > 0,
        "trial copy did not enter native active state",
    )?;
    plugins::plugin_views::revoke_window(&trial.session_id);
    let task = native_done(app, trial_id).await?;
    require(
        task.state == games::tasks::TaskState::Succeeded,
        "trial native copy failed after view close",
    )?;
    require(task.has_result, "trial copy has no Task Center result")?;
    let copied_id = task
        .game_id
        .as_deref()
        .ok_or("trial did not register new game")?;
    let copied = games::workspace::snapshot(app, copied_id)?;
    require(
        copied
            .directory
            .canonicalize()
            .map_err(io)?
            .starts_with(base.join("copies").canonicalize().map_err(io)?),
        "trial escaped isolated copy root",
    )?;
    require(
        !copied.directory.join("saves").exists() && !copied.directory.join("logs").exists(),
        "trial default copied excluded worlds/logs",
    )?;
    std::fs::write(
        copied.directory.join("options.txt"),
        b"changed only in trial",
    )
    .map_err(io)?;
    require(
        std::fs::read(source.join("options.txt")).map_err(io)? == b"synthetic:true\n",
        "trial and source share writable bytes",
    )?;
    // A real active copy is cancelled and awaited; the source remains usable.
    eprintln!("MAINTENANCE_STAGE trial-cancel");
    let trial = auth(app, "trial-copy", "trial-cancel")?;
    let cancel_preview = games::trial_copy::isolated_preview(
        app.clone(),
        &trial,
        request(json!({"gameId":GAME,"name":"Cancel synthetic trial"}))?,
        base.join("copies"),
    )
    .await?;
    let cancellation = games::trial_copy::start(
        app.clone(),
        &trial,
        request(json!({"planId":string(&cancel_preview,"planId")?}))?,
    )
    .await?;
    let cancel_id = string(&cancellation, "taskId")?;
    games::tasks::cancel_task(app.clone(), cancel_id.into())?;
    let cancelled = trial_done(app, &trial, cancel_id).await?;
    require(
        cancelled["status"] == "cancelled",
        "active trial cancellation did not terminate as cancelled",
    )?;
    require(
        games::tasks::active_count(app) == 0,
        "cancelled task still holds native active slot",
    )?;
    let cancelled_record = native_done(app, cancel_id).await?;
    require(
        cancelled_record.state == games::tasks::TaskState::Cancelled,
        "Task Center did not record native cancellation",
    )?;
    require(
        source.join("saves/synthetic/level.dat").is_file(),
        "trial cancellation removed source world",
    )?;
    // Real move plan, copy, verification and library publication. No source recycling here.
    eprintln!("MAINTENANCE_STAGE migration");
    let move_preview = games::storage_workspace::isolated_plan(
        app.clone(),
        &storage,
        base.join("move-destination"),
        GAME.into(),
    )
    .await?;
    let moving = games::storage_workspace::start_move(
        app.clone(),
        &storage,
        request(json!({"planId":string(&move_preview,"planId")?}))?,
    )
    .await?;
    let move_id = string(&moving, "taskId")?;
    require(
        games::tasks::active_count(app) > 0,
        "migration did not enter native active state",
    )?;
    plugins::plugin_views::revoke_window(&storage.session_id);
    let moved = native_done(app, move_id).await?;
    require(
        moved.state == games::tasks::TaskState::Succeeded,
        "native migration failed after view close",
    )?;
    require(
        moved.has_result && moved.game_id.as_deref() == Some(GAME),
        "migration Task Center result is not navigable",
    )?;
    let current = games::workspace::snapshot(app, GAME)?;
    require(
        current.directory.canonicalize().map_err(io)?
            == base
                .join("move-destination")
                .join(GAME)
                .canonicalize()
                .map_err(io)?,
        "migration did not publish exact selected destination",
    )?;
    require(
        source.join("saves/synthetic/level.dat").is_file()
            && current
                .directory
                .join("saves/synthetic/level.dat")
                .is_file(),
        "migration lost source or copied world",
    )?;
    require(
        std::fs::read(current.directory.join("saves/synthetic/level.dat")).map_err(io)?
            == b"ORIGINAL_SYNTHETIC_WORLD",
        "migration world bytes changed",
    )?;
    let persisted: Value = serde_json::from_slice(
        &std::fs::read(
            crate::config_dir()
                .ok_or("config root")?
                .join("CobbleLauncher/games.json"),
        )
        .map_err(io)?,
    )
    .map_err(io)?;
    require(
        persisted.as_array().is_some_and(|rows| {
            rows.iter().any(|row| {
                row["id"] == GAME
                    && row["directory"] == current.directory.to_string_lossy().as_ref()
            })
        }),
        "migration library publication did not persist",
    )?;
    // Config backup-before-write and restore through the actual permission/revision adapter.
    eprintln!("MAINTENANCE_STAGE config-restore");
    let listed = plugins::plugin_config_workspace::list(
        app.clone(),
        config.clone(),
        request(json!({"gameId":GAME}))?,
    )
    .await?;
    let file_id = listed
        .as_array()
        .and_then(|a| a.first())
        .and_then(|r| r["id"].as_str())
        .ok_or("config listing empty")?;
    let read = plugins::plugin_config_workspace::read(
        app.clone(),
        config.clone(),
        request(json!({"gameId":GAME,"configId":file_id}))?,
    )
    .await?;
    let written=plugins::plugin_config_workspace::write(app.clone(),config.clone(),request(json!({"gameId":GAME,"configId":file_id,"expectedRevision":string(&read,"revision")?,"text":"{\"volume\":2}\n"}))?).await?;
    let backup = written["backups"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|v| v["id"].as_str())
        .ok_or("config write did not retain backup")?;
    let restored=plugins::plugin_config_workspace::restore(app.clone(),config.clone(),request(json!({"gameId":GAME,"configId":file_id,"backupId":backup,"expectedRevision":string(&written,"revision")?}))?).await?;
    require(
        restored["text"] == "{\"volume\":1}\n",
        "config restore did not restore original bytes",
    )?;
    std::fs::write(
        current.directory.join("config/synthetic.json"),
        b"{\"volume\":99}\n",
    )
    .map_err(io)?;
    let stale=plugins::plugin_config_workspace::restore(app.clone(),config,request(json!({"gameId":GAME,"configId":file_id,"backupId":backup,"expectedRevision":string(&restored,"revision")?}))?).await;
    require(
        matches!(stale,Err(ref error) if error=="CONFIG_CHANGED"),
        "config restore accepted a stale revision",
    )?;
    require(
        std::fs::read(current.directory.join("config/synthetic.json")).map_err(io)?
            == b"{\"volume\":99}\n",
        "stale config restore overwrote later edits",
    )?;
    // Real native share service and real MRPACK parser, with no provider network fixture.
    eprintln!("MAINTENANCE_STAGE share-archive");
    let share_preview = plugins::plugin_play_manifest::preview(
        app.clone(),
        &share,
        request(json!({"gameId":GAME}))?,
    )
    .await?;
    require(
        share_preview["mods"].as_array().is_some_and(Vec::is_empty),
        "unverified jar became a shared reference",
    )?;
    require(
        share_preview["excludedMods"]
            .as_array()
            .is_some_and(|m| m.iter().any(|v| v == "unverified.jar")),
        "unverified jar omission not disclosed",
    )?;
    let selected = share_preview["configs"]
        .as_array()
        .ok_or("share config catalog")?
        .iter()
        .map(|v| v["id"].clone())
        .collect::<Vec<_>>();
    let archive = base.join("synthetic-share.mrpack");
    let saved=plugins::plugin_play_manifest::isolated_export(app.clone(),&share,request(json!({"gameId":GAME,"revision":string(&share_preview,"revision")?,"configIds":selected}))?,archive.clone()).await?;
    require(saved["saved"] == true, "native archive export did not save")?;
    cobble_content::read_mrpack(&archive).map_err(io)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&archive).map_err(io)?).map_err(io)?;
    require(zip.len() == 2, "share archive included unselected files")?;
    require(
        zip.by_name("overrides/config/synthetic.json").is_ok(),
        "selected config not included",
    )?;
    require(
        zip.by_name("overrides/mods/unverified.jar").is_err()
            && zip.by_name("overrides/accounts.json").is_err(),
        "share archive embedded private or unverified content",
    )?;
    // A selected source is snapshotted, classified and explicitly handed to the main native adapter.
    eprintln!("MAINTENANCE_STAGE inbox-handoff");
    use std::io::Write;
    let pack = base.join("synthetic-pack.zip");
    {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&pack).map_err(io)?);
        writer
            .start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .map_err(io)?;
        writer
            .write_all(
                b"{\"pack\":{\"pack_format\":34,\"description\":\"Original synthetic fixture\"}}",
            )
            .map_err(io)?;
        writer
            .start_file(
                "assets/synthetic/lang/en_us.json",
                zip::write::SimpleFileOptions::default(),
            )
            .map_err(io)?;
        writer
            .write_all(b"{\"synthetic.test\":\"Original fixture\"}")
            .map_err(io)?;
        writer.finish().map_err(io)?;
    }
    let expected_pack = std::fs::read(&pack).map_err(io)?;
    let event = Arc::new(Mutex::new(None::<String>));
    let events = event.clone();
    let listener = app.listen_any("plugin-file-inbox-open", move |e| {
        if let Ok(v) = serde_json::from_str::<Value>(e.payload()) {
            *events.lock().unwrap() = v["handoff_id"].as_str().map(str::to_owned);
        }
    });
    require(
        plugins::plugin_file_inbox::capture_drop(app.clone(), vec![pack]).await?,
        "native dropped-file capture was not accepted",
    )?;
    let pending = plugins::plugin_file_inbox::pending(app.clone(), &inbox).await?;
    let item = pending
        .as_array()
        .and_then(|rows| rows.first())
        .ok_or("native inbox did not retain resource")?;
    let resource = string(item, "resourceId")?;
    require(
        item["handlers"]
            .as_array()
            .is_some_and(|h| h.iter().any(|v| v["id"] == "builtin:resource-pack")),
        "resource pack had no genuine native import handler",
    )?;
    plugins::plugin_file_inbox::open(
        app.clone(),
        &inbox,
        request(json!({"resourceId":resource,"handlerId":"builtin:resource-pack","gameId":GAME}))?,
    )
    .await?;
    let handoff = event
        .lock()
        .map_err(|_| "handoff event poisoned")?
        .clone()
        .ok_or("main host review handoff event missing")?;
    app.unlisten(listener);
    let view: tauri::Webview = window.as_ref().clone();
    let review = plugins::plugin_file_inbox::file_inbox_preview_handoff(
        app.clone(),
        view.clone(),
        handoff.clone(),
    )
    .await?;
    require(
        review["handler"]["id"] == "builtin:resource-pack" && review["game"]["id"] == GAME,
        "main preview changed selected import target",
    )?;
    let imported = plugins::plugin_file_inbox::file_inbox_apply_handoff(
        app.clone(),
        view.clone(),
        handoff.clone(),
        true,
    )
    .await?;
    require(
        imported["imported"] == true && imported["gameId"] == GAME,
        "native inbox handoff did not import",
    )?;
    require(
        current
            .directory
            .join("resourcepacks/synthetic-pack.zip")
            .is_file(),
        "actual selected pack was not written",
    )?;
    require(
        std::fs::read(current.directory.join("resourcepacks/synthetic-pack.zip")).map_err(io)?
            == expected_pack,
        "native inbox import changed selected snapshot bytes",
    )?;
    require(
        plugins::plugin_file_inbox::file_inbox_apply_handoff(app.clone(), view, handoff, true)
            .await
            .is_err(),
        "native inbox token replay succeeded",
    )?;
    Ok(
        json!({"canonicalPlugins":packages,"trialCopy":{"independent":true,"worldsExcludedByDefault":true,"activeCancellationAwaited":true},"migration":{"actualCopyAndLibraryPublication":true,"sourceRetained":true,"worldBytesPreserved":true},"configRestore":{"backupBeforeWrite":true,"restored":true,"laterEditPreserved":true},"shareArchive":{"nativeService":true,"actualMrpackParser":true,"unknownJarExcluded":true,"selectedConfigsOnly":true,"providerNetworkTested":false},"fileInbox":{"actualCaptureSnapshot":true,"mainHandoffPreview":true,"actualImport":true,"singleUse":true},"osDialogsOperated":false}),
    )
}

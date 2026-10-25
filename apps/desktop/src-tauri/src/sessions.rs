// SPDX-License-Identifier: MIT
//! Host-only process identity. Command lines are inspected in memory, never saved.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Session {
    pub game_id: String,
    pub directory: PathBuf,
    pub natives: PathBuf,
    pub pid: u32,
    pub started: u64,
    pub executable: PathBuf,
}

fn process(pid: u32) -> System {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always),
    );
    system
}
fn session_argument(natives: &Path) -> String {
    format!("-Dcobble.launcher.session={}", natives.display())
}
fn matches(session: &Session, system: &System) -> bool {
    system
        .process(Pid::from_u32(session.pid))
        .is_some_and(|process| {
            process.start_time() == session.started
                && process.exe() == Some(session.executable.as_path())
                && process
                    .cmd()
                    .iter()
                    .any(|argument| argument == session_argument(&session.natives).as_str())
                && !matches!(
                    process.status(),
                    sysinfo::ProcessStatus::Zombie | sysinfo::ProcessStatus::Dead
                )
        })
}
pub(super) fn alive(session: &Session) -> bool {
    matches(session, &process(session.pid))
}
pub(super) fn capture(game: &super::Game, pid: u32, natives: &Path) -> Result<Session, String> {
    let system = process(pid);
    let process = system.process(Pid::from_u32(pid)).ok_or("游戏进程已退出")?;
    let session = Session {
        game_id: game.id.clone(),
        directory: game.directory.clone(),
        natives: natives.into(),
        pid,
        started: process.start_time(),
        executable: process.exe().ok_or("无法读取游戏进程身份")?.into(),
    };
    if !matches(&session, &system) {
        return Err("无法确认游戏进程身份，请重试启动".into());
    }
    Ok(session)
}
fn record_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("无效的游戏会话 ID".into());
    }
    Ok(dirs::config_dir()
        .ok_or("无法找到配置目录")?
        .join("CobbleLauncher/running-games")
        .join(format!("{id}.json")))
}
fn read(id: &str) -> Result<Option<Session>, String> {
    read_at(&record_path(id)?)
}
fn read_at(file: &Path) -> Result<Option<Session>, String> {
    match fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| "无法读取游戏运行记录，请检查本地配置".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}
pub(super) fn persist(session: &Session) -> Result<(), String> {
    let file = record_path(&session.game_id)?;
    persist_at(session, &file)
}
fn persist_at(session: &Session, file: &Path) -> Result<(), String> {
    fs::create_dir_all(file.parent().ok_or("无法找到会话目录")?)
        .map_err(|error| error.to_string())?;
    let temporary = file.with_extension("json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec(session).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(temporary, file).map_err(|error| error.to_string())
}
pub(super) fn recover(game: &super::Game) -> Result<Option<Session>, String> {
    let Some(session) = read(&game.id)? else {
        return Ok(None);
    };
    recover_record(game, session)
}
fn recover_record(game: &super::Game, session: Session) -> Result<Option<Session>, String> {
    validate_record(game, &session)?;
    Ok(alive(&session).then_some(session))
}

pub(super) fn clear(session: &Session) -> Result<(), String> {
    if let Some(current) = read(&session.game_id)?
        && current.pid == session.pid
        && current.started == session.started
        && current.natives == session.natives
    {
        fs::remove_file(record_path(&session.game_id)?).map_err(|error| error.to_string())?;
    }
    Ok(())
}
pub(crate) fn watch_recovered(app: &AppHandle) {
    let games = {
        let library = app.state::<super::GameLibrary>();
        let library = library.0.lock().expect("game library mutex");
        library
            .games
            .iter()
            .filter(|game| game.launch_state == "verifying")
            .cloned()
            .collect::<Vec<_>>()
    };
    let app = app.clone();
    std::thread::spawn(move || recover_loaded(&app, games));
}

/// One process refresh for the recorded PIDs, never a full-machine process scan.
fn recover_batch(games: &[super::Game]) -> Vec<(String, Result<Option<Session>, String>)> {
    recover_batch_with(games, |game| read(&game.id))
}
fn recover_batch_with(
    games: &[super::Game],
    mut read_record: impl FnMut(&super::Game) -> Result<Option<Session>, String>,
) -> Vec<(String, Result<Option<Session>, String>)> {
    let records = games
        .iter()
        .map(|game| (game, read_record(game)))
        .collect::<Vec<_>>();
    let mut pids = records
        .iter()
        .filter_map(|(_, record)| {
            record
                .as_ref()
                .ok()
                .and_then(|record| record.as_ref())
                .map(|session| Pid::from_u32(session.pid))
        })
        .collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    let mut system = System::new();
    if !pids.is_empty() {
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing()
                .with_exe(UpdateKind::Always)
                .with_cmd(UpdateKind::Always),
        );
    }
    records
        .into_iter()
        .map(|(game, record)| {
            let result = record.and_then(|record| match record {
                None => Ok(None),
                Some(session) => {
                    validate_record(game, &session)?;
                    Ok(matches(&session, &system).then_some(session))
                }
            });
            (game.id.clone(), result)
        })
        .collect()
}

fn validate_record(game: &super::Game, session: &Session) -> Result<(), String> {
    if session.game_id != game.id || session.directory != game.directory {
        return Err("游戏运行记录与实例不一致".into());
    }
    Ok(())
}

fn recover_loaded(app: &AppHandle, games: Vec<super::Game>) {
    let started = std::time::Instant::now();
    for (id, result) in recover_batch(&games) {
        let session = result.as_ref().ok().and_then(|session| session.clone());
        let state = app.state::<super::GameLibrary>();
        let Ok(mut library) = state.0.lock() else {
            return;
        };
        let Some(game) = library
            .games
            .iter_mut()
            .find(|game| game.id == id && game.launch_state == "verifying")
        else {
            continue;
        };
        // A deferred result never authorizes a changed instance directory.
        if !games
            .iter()
            .any(|original| original.id == id && original.directory == game.directory)
        {
            continue;
        }
        match result {
            Ok(Some(_)) => {
                game.launch_state = "running".into();
            }
            Ok(None) => {
                game.launch_state = super::idle();
            }
            Err(error) => {
                game.launch_state = "recovery_failed".into();
                game.launch_error = Some(error);
            }
        }
        let _ = app.emit("game-install", &*game);
        drop(library);
        if let Some(session) = session {
            watch_session(app, session);
        }
    }
    tracing::info!(
        event = "startup_session_recovery_complete",
        elapsed_ms = started.elapsed().as_millis() as u64,
        instances = games.len()
    );
    super::world_snapshots::automatic::recover_pending(app);
}

#[tauri::command]
pub async fn retry_game_session_recovery(app: AppHandle, id: String) -> Result<(), String> {
    let game = {
        let state = app.state::<super::GameLibrary>();
        let mut library = state.0.lock().map_err(|error| error.to_string())?;
        let game = library
            .games
            .iter_mut()
            .find(|game| game.id == id)
            .ok_or("找不到这个游戏")?;
        if game.launch_state != "recovery_failed" {
            return Err("这个游戏无需重试运行检查".into());
        }
        game.launch_state = "verifying".into();
        game.launch_error = None;
        let _ = app.emit("game-install", &*game);
        game.clone()
    };
    tauri::async_runtime::spawn_blocking(move || recover_loaded(&app, vec![game]))
        .await
        .map_err(|error| error.to_string())
}

fn watch_session(app: &AppHandle, session: Session) {
    let app = app.clone();
    std::thread::spawn(move || {
        while alive(&session) {
            std::thread::sleep(Duration::from_secs(1));
        }
        tauri::async_runtime::block_on(super::world_snapshots::automatic::after_exit(
            &app, &session,
        ));
        let _ = clear(&session);
        super::update_game(
            &app,
            &session.game_id,
            |game| {
                if game.launch_state == "running" {
                    game.launch_state = super::idle();
                }
            },
            false,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command, Stdio};
    struct Fixture {
        root: PathBuf,
        child: Child,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn fixture() -> (Fixture, super::super::Game) {
        let root = std::env::temp_dir().join(format!(
            "cobble-session-{}-{}",
            std::process::id(),
            super::super::next_pack_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "games::sessions::tests::owned_process_fixture",
                "--skip",
                &session_argument(&root),
            ])
            .env("COBBLE_OWNED_SESSION_FIXTURE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let child = command.spawn().unwrap();
        let game = serde_json::from_value(serde_json::json!({"id":"ab", "name":"test", "version":"1.21.1", "directory":root, "status":"installed"})).unwrap();
        (Fixture { root, child }, game)
    }
    #[test]
    fn owned_process_fixture() {
        if std::env::var_os("COBBLE_OWNED_SESSION_FIXTURE").is_some() {
            std::thread::sleep(Duration::from_secs(20));
        }
    }
    #[test]
    fn persisted_owned_process_recovers_and_exit_releases_state() {
        let (mut fixture, game) = fixture();
        let session = capture(&game, fixture.child.id(), &fixture.root).unwrap();
        let file = fixture.root.join("session.json");
        persist_at(&session, &file).unwrap();
        let loaded = read_at(&file).unwrap().unwrap();
        assert!(recover_record(&game, loaded.clone()).unwrap().is_some());
        let saved: serde_json::Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        assert_eq!(saved.as_object().unwrap().len(), 6);
        assert!(saved.get("cmd").is_none());
        fixture.child.kill().unwrap();
        fixture.child.wait().unwrap();
        assert!(recover_record(&game, loaded).unwrap().is_none());
    }
    #[test]
    fn pid_reuse_or_wrong_session_is_not_claimed_as_running_game() {
        let (fixture, game) = fixture();
        let session = capture(&game, fixture.child.id(), &fixture.root).unwrap();
        let mut reused = session.clone();
        reused.started += 1;
        assert!(!alive(&reused));
        let mut other = session.clone();
        other.natives = fixture.root.join("different-launch");
        assert!(!alive(&other));
        let mut foreign = session;
        foreign.directory = fixture.root.join("different-game");
        assert!(recover_record(&game, foreign).is_err());
        assert!(capture(&game, fixture.child.id(), &fixture.root.join("unrelated")).is_err());
    }

    #[test]
    fn deferred_batch_isolates_bad_record_and_recovers_owned_process() {
        let (fixture, game) = fixture();
        let session = capture(&game, fixture.child.id(), &fixture.root).unwrap();
        let mut bad = game.clone();
        bad.id = "bad".into();
        let mut absent = game.clone();
        absent.id = "a0".into();
        let results = recover_batch_with(&[game.clone(), bad, absent], |item| {
            match item.id.as_str() {
                "ab" => Ok(Some(session.clone())),
                "bad" => Err("synthetic unreadable record".into()),
                _ => Ok(None),
            }
        });
        assert!(results[0].1.as_ref().unwrap().is_some());
        assert!(results[1].1.is_err());
        assert!(results[2].1.as_ref().unwrap().is_none());
        assert_eq!(results[0].0, game.id);
    }

    #[test]
    #[ignore = "explicit synthetic recovery timing, not a native startup benchmark"]
    fn synthetic_recovery_measurement() {
        let (fixture, game) = fixture();
        let session = capture(&game, fixture.child.id(), &fixture.root).unwrap();
        let games = (0..32)
            .map(|index| {
                let mut item = game.clone();
                item.id = format!("{index:x}");
                item
            })
            .collect::<Vec<_>>();
        let records = games
            .iter()
            .map(|item| {
                let mut record = session.clone();
                record.game_id = item.id.clone();
                record
            })
            .collect::<Vec<_>>();
        let mut samples = Vec::new();
        for _ in 0..7 {
            let before = std::time::Instant::now();
            for (item, record) in games.iter().zip(&records) {
                assert!(recover_record(item, record.clone()).unwrap().is_some());
            }
            let per_record_us = before.elapsed().as_micros();
            let before = std::time::Instant::now();
            let batch = recover_batch_with(&games, |item| {
                Ok(Some(
                    records
                        .iter()
                        .find(|record| record.game_id == item.id)
                        .unwrap()
                        .clone(),
                ))
            });
            assert!(
                batch
                    .iter()
                    .all(|(_, result)| result.as_ref().is_ok_and(|session| session.is_some()))
            );
            samples.push(serde_json::json!({"per_record_us":per_record_us,"batched_us":before.elapsed().as_micros()}));
        }
        println!(
            "RECOVERY_MEASUREMENT {}",
            serde_json::json!({"scenario":"32 synthetic records referencing one owned helper process; in-memory records; excludes file reads, native WebView, and first-screen render", "samples":samples})
        );
    }
}

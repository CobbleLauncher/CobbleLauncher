// SPDX-License-Identifier: MIT
use super::*;
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Serialize)]
pub struct LaunchIntent {
    queue_id: String,
    game_id: String,
    play: bool,
}
static PENDING: OnceLock<Mutex<Option<LaunchIntent>>> = OnceLock::new();
static NEXT: AtomicU64 = AtomicU64::new(1);
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn parse(arguments: impl Iterator<Item = String>) -> Option<(String, bool)> {
    let args = arguments.collect::<Vec<_>>();
    if args.len() > 3 {
        return None;
    }
    let mut id = None;
    let mut play = false;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--instance" if id.is_none() => {
                let value = args.next()?;
                if !valid_id(&value) {
                    return None;
                }
                id = Some(value);
            }
            "--play" if !play => play = true,
            _ => return None,
        }
    }
    Some((id?, play))
}
pub fn queue_arguments(arguments: impl Iterator<Item = String>) -> bool {
    let Some((game_id, play)) = parse(arguments) else {
        return false;
    };
    let Ok(mut pending) = PENDING.get_or_init(Default::default).lock() else {
        return false;
    };
    *pending = Some(LaunchIntent {
        queue_id: NEXT.fetch_add(1, Ordering::Relaxed).to_string(),
        game_id,
        play,
    });
    true
}
#[tauri::command]
pub fn peek_pending_launch(
    library: State<'_, GameLibrary>,
) -> Result<Option<LaunchIntent>, String> {
    let mut pending = PENDING
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "LAUNCH_INTENT_UNAVAILABLE")?;
    if let Some(intent) = pending.as_ref() {
        let library = library.0.lock().map_err(|_| "LAUNCH_INTENT_UNAVAILABLE")?;
        if !library
            .games
            .iter()
            .any(|game| game.id == intent.game_id && game.status == "installed")
        {
            *pending = None;
            return Err("QUICK_LAUNCH_GAME_UNAVAILABLE".into());
        }
    }
    Ok(pending.clone())
}
#[tauri::command]
pub fn acknowledge_pending_launch(queue_id: String) -> Result<bool, String> {
    let mut pending = PENDING
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| "LAUNCH_INTENT_UNAVAILABLE")?;
    if pending
        .as_ref()
        .is_some_and(|intent| intent.queue_id == queue_id)
    {
        *pending = None;
        Ok(true)
    } else {
        Ok(false)
    }
}
#[tauri::command]
pub async fn create_game_shortcut(app: AppHandle, id: String) -> Result<(), String> {
    let name = {
        let library = app.state::<GameLibrary>();
        let library = library.0.lock().map_err(|e| e.to_string())?;
        if !valid_id(&id)
            || !library
                .games
                .iter()
                .any(|game| game.id == id && game.status == "installed")
        {
            return Err("QUICK_LAUNCH_GAME_UNAVAILABLE".into());
        }
        library
            .games
            .iter()
            .find(|game| game.id == id)
            .ok_or("QUICK_LAUNCH_GAME_UNAVAILABLE")?
            .name
            .clone()
    };
    if !cfg!(target_os = "windows") {
        return Err("SHORTCUT_UNSUPPORTED_PLATFORM".into());
    }
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let desktop = dirs::desktop_dir().ok_or("DESKTOP_UNAVAILABLE")?;
    let target = desktop.join(shortcut_file_name(&name, &id));
    tauri::async_runtime::spawn_blocking(move || {
        cobble_platform::create_launch_shortcut(&executable, &target, &id)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn shortcut_file_name(name: &str, id: &str) -> String {
    let safe: String = name
        .chars()
        .filter(|ch| {
            !ch.is_control() && !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        .take(60)
        .collect();
    let safe = safe.trim().trim_matches('.').trim();
    let safe = if safe.is_empty() { "Minecraft" } else { safe };
    let suffix: String = id.chars().take(8).collect();
    format!("{safe} - CobbleLauncher {suffix}.lnk")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_names_are_human_readable_and_never_paths() {
        assert_eq!(
            shortcut_file_name("我的整合包", "abcdef012345"),
            "我的整合包 - CobbleLauncher abcdef01.lnk"
        );
        assert_eq!(
            shortcut_file_name(" ../Bad:<Name>*? ", "abc"),
            "BadName - CobbleLauncher abc.lnk"
        );
        assert_eq!(
            shortcut_file_name("...", "abc"),
            "Minecraft - CobbleLauncher abc.lnk"
        );
    }
    fn args(values: &[&str]) -> impl Iterator<Item = String> {
        values
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }
    #[test]
    fn strict_intent_parser_preserves_legacy_ids_and_rejects_paths_and_extra_switches() {
        assert_eq!(
            parse(args(&["--instance", "legacy_game-1", "--play"])),
            Some(("legacy_game-1".into(), true))
        );
        assert_eq!(parse(args(&["--instance", "a"])), Some(("a".into(), false)));
        for values in [
            vec!["--instance", "../a", "--play"],
            vec!["--play"],
            vec!["--instance", "a", "--exec"],
            vec!["--instance", "a", "--instance", "b"],
        ] {
            assert!(parse(args(&values)).is_none());
        }
    }
    #[test]
    fn stale_ack_does_not_drop_a_new_intent() {
        assert!(queue_arguments(args(&["--instance", "first", "--play"])));
        let first = PENDING
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .queue_id
            .clone();
        assert!(queue_arguments(args(&["--instance", "second", "--play"])));
        assert!(!acknowledge_pending_launch(first).unwrap());
        assert_eq!(
            PENDING
                .get()
                .unwrap()
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .game_id,
            "second"
        );
    }
}

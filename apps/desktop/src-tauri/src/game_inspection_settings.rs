// SPDX-License-Identifier: MIT
use super::*;
fn options(path: &Path) -> Result<Vec<(String, String)>, String> {
    let raw = text(&path.join("options.txt"))?.unwrap_or_default();
    if raw.lines().count() > LIMIT {
        return Err("Options entry limit exceeded".into());
    }
    Ok(raw
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}
pub(super) fn selected_packs(path: &Path) -> Option<Vec<String>> {
    let value = options(path)
        .ok()?
        .into_iter()
        .find(|(key, _)| key == "resourcePacks")?
        .1;
    let list: Vec<String> = serde_json::from_str(&value).ok()?;
    (list.len() <= LIMIT).then(|| list.into_iter().map(|value| clean(&value)).collect())
}
fn allowed(key: &str) -> bool {
    key.starts_with("key_")
        || matches!(
            key,
            "sensitivity"
                | "invertYMouse"
                | "touchscreen"
                | "autoJump"
                | "toggleCrouch"
                | "toggleSprint"
                | "rawMouseInput"
                | "mouseWheelSensitivity"
                | "discrete_mouse_scroll"
                | "fullscreen"
                | "fov"
                | "renderDistance"
                | "simulationDistance"
                | "guiScale"
                | "maxFps"
                | "vsync"
        )
}
pub(super) fn collect(game: &Game, scope: &str) -> Result<Value, String> {
    let items = if scope == "runtime" {
        let runtime = &game.runtime_overrides;
        let defaults = cobble_minecraft::read_launcher_preferences()
            .map_err(|_| "Launcher preferences unavailable")?;
        let required = required_major(game);
        let selected = crate::java::cached_major(&runtime.java, required);
        let java_mode = match &runtime.java {
            cobble_minecraft::JavaOverride::Inherit => "inherit",
            cobble_minecraft::JavaOverride::Automatic => "automatic",
            cobble_minecraft::JavaOverride::Custom { .. } => "custom",
        };
        vec![
            json!({"name":"minecraftVersion","value":clean(&game.version)}),
            json!({"name":"loader","value":game.loader_kind.as_ref().map(|v|clean(v))}),
            json!({"name":"loaderVersion","value":game.loader_version.as_ref().map(|v|clean(v))}),
            json!({"name":"memoryMb","value":runtime.memory_mb.unwrap_or(defaults.memory_mb)}),
            json!({"name":"javaMode","value":java_mode}),
            json!({"name":"requiredJavaMajor","value":required}),
            json!({"name":"selectedJavaMajor","value":selected}),
            json!({"name":"launchState","value":game.launch_state}),
            json!({"name":"jvmArgumentCount","value":runtime.jvm_args.as_ref().map(Vec::len).unwrap_or(defaults.jvm_args.len())}),
        ]
    } else {
        options(&game.directory)?
            .into_iter()
            .filter(|(key, _)| allowed(key))
            .map(|(key, value)| json!({"name":clean(&key),"value":clean(&value)}))
            .collect()
    };
    Ok(json!({"items":items,"available":true,"truncated":false}))
}
fn required_major(game: &Game) -> Option<u32> {
    if game.version.is_empty()
        || !game
            .version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return None;
    }
    let versions = game.directory.join("versions");
    let version = versions.join(&game.version);
    if !directory(&versions).ok()? || !directory(&version).ok()? {
        return None;
    }
    let raw = text(&version.join(format!("{}.json", game.version))).ok()??;
    let metadata: Value = serde_json::from_str(&raw).ok()?;
    metadata
        .get("javaVersion")?
        .get("majorVersion")?
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn option_allowlist_excludes_private_settings() {
        assert!(allowed("key_key.forward"));
        assert!(allowed("sensitivity"));
        assert!(!allowed("lastServer"));
        assert!(!allowed("chatFormat"));
    }
}

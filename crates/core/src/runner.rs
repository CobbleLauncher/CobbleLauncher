// SPDX-License-Identifier: MIT
//! Explicit local offline-profile launch. This does not authenticate an account.
use crate::{LaunchPreparation, prepare_launch};
use cobble_auth::OfflineProfile;
use cobble_minecraft::{
    LaunchOverrides, read_launcher_preferences, resolve_java_override, substitute,
    validate_memory_mb,
};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::{Child, Command};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// The caller owns the child and can wait for its exit. Dropping this handle does
/// not terminate Minecraft. Session native files remain available to the JVM.
pub struct RunningGame {
    pub child: Child,
    pub log_path: PathBuf,
    pub natives_directory: PathBuf,
}

pub async fn start_offline_game(
    version: &str,
    directory: &Path,
    profile: &OfflineProfile,
) -> Result<RunningGame, BoxError> {
    start_offline_game_with_options(version, directory, profile, &LaunchOverrides::default()).await
}

pub async fn start_offline_game_with_options(
    version: &str,
    directory: &Path,
    profile: &OfflineProfile,
    options: &LaunchOverrides,
) -> Result<RunningGame, BoxError> {
    let memory_mb = match options.memory_mb {
        Some(memory_mb) => memory_mb,
        None => read_launcher_preferences()?.memory_mb,
    };
    validate_memory_mb(memory_mb)?;
    let plan = prepare_launch(version, directory).await?;
    let java = resolve_java_override(plan.required_java, &options.java).await?;
    let arguments = prepare_offline_arguments(&plan, profile, memory_mb)?;
    let directory = dunce::canonicalize(directory)?;
    let logs = directory.join(".cobble-logs");
    fs::create_dir_all(&logs)?;
    let log_path = logs.join("launcher.log");
    let output = fs::File::create(&log_path)?;
    let mut command = Command::new(java.executable);
    command
        .args(arguments)
        .current_dir(directory)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output));
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // Hide the Java console, not its game window.
    let child = command.spawn()?;
    Ok(RunningGame {
        child,
        log_path,
        natives_directory: plan.natives_directory,
    })
}

/// Prepare argv without spawning a game or accessing launcher configuration.
pub fn prepare_offline_arguments(
    plan: &LaunchPreparation,
    profile: &OfflineProfile,
    memory_mb: u32,
) -> Result<Vec<String>, BoxError> {
    validate_memory_mb(memory_mb)?;
    // Public fields may have been constructed by a caller: revalidate the name
    // and deterministic offline UUID before passing them to a game process.
    if OfflineProfile::new(&profile.name)? != *profile {
        return Err("offline profile UUID does not match its name".into());
    }
    let placeholders = HashMap::from([
        ("auth_player_name".into(), profile.name.clone()),
        ("auth_uuid".into(), profile.uuid.clone()),
        ("auth_access_token".into(), "0".into()),
        ("auth_session".into(), "0".into()),
        ("user_type".into(), "legacy".into()),
        ("user_properties".into(), "{}".into()),
        ("auth_xuid".into(), String::new()),
    ]);
    let bind = |value: &str| -> Result<String, BoxError> {
        let value = substitute(value, &placeholders)?;
        if value.contains("${") {
            return Err("unresolved launch argument placeholder".into());
        }
        Ok(value)
    };
    let mut arguments = vec![format!("-Xmx{memory_mb}M")];
    for argument in &plan.jvm_arguments {
        if !argument.starts_with("-Xmx") {
            arguments.push(bind(argument)?);
        }
    }
    arguments.push(plan.main_class.clone());
    for argument in &plan.game_arguments {
        arguments.push(bind(argument)?);
    }
    Ok(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> LaunchPreparation {
        LaunchPreparation {
            required_java: 21,
            main_class: "net.minecraft.client.main.Main".into(),
            classpath_entries: 1,
            classpath: vec![],
            jvm_arguments: vec!["-cp".into(), "C:/Game folder/client.jar".into()],
            game_arguments: [
                "--username",
                "${auth_player_name}",
                "--uuid",
                "${auth_uuid}",
                "--accessToken",
                "${auth_access_token}",
                "${auth_session}",
                "${user_type}",
                "${user_properties}",
                "${auth_xuid}",
                "two words",
            ]
            .map(String::from)
            .into(),
            natives_directory: PathBuf::new(),
            native_files: 0,
        }
    }

    #[test]
    fn binds_only_explicit_offline_values_preserving_argv_and_spaces() {
        let profile = OfflineProfile::new("Notch").unwrap();
        let arguments = prepare_offline_arguments(&plan(), &profile, 2048).unwrap();
        assert_eq!(
            arguments,
            [
                "-Xmx2048M",
                "-cp",
                "C:/Game folder/client.jar",
                "net.minecraft.client.main.Main",
                "--username",
                "Notch",
                "--uuid",
                "b50ad385829d3141a2167e7d7539ba7f",
                "--accessToken",
                "0",
                "0",
                "legacy",
                "{}",
                "",
                "two words"
            ]
        );
        assert!(arguments.iter().all(|argument| !argument.contains("${")));
    }

    #[test]
    fn replaces_all_metadata_heaps_and_rejects_unbound_or_tampered_profile() {
        let profile = OfflineProfile::new("Notch").unwrap();
        let mut plan = plan();
        plan.jvm_arguments.push("-Xmx1024M".into());
        plan.jvm_arguments.push("-Xmx8G".into());
        let arguments = prepare_offline_arguments(&plan, &profile, 4096).unwrap();
        assert_eq!(
            arguments
                .iter()
                .filter(|argument| argument.starts_with("-Xmx"))
                .map(String::as_str)
                .collect::<Vec<_>>(),
            vec!["-Xmx4096M"]
        );
        assert!(prepare_offline_arguments(&plan, &profile, 511).is_err());
        plan.game_arguments.push("${clientid}".into());
        assert!(prepare_offline_arguments(&plan, &profile, 2048).is_err());
        plan.game_arguments.pop();
        plan.game_arguments.push("${unknown}".into());
        assert!(prepare_offline_arguments(&plan, &profile, 2048).is_err());
        plan.game_arguments.pop();
        let mut profile = profile;
        profile.uuid = "0".repeat(32);
        assert!(prepare_offline_arguments(&plan, &profile, 2048).is_err());
    }
}

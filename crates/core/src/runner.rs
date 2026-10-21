// SPDX-License-Identifier: MIT
//! Explicit local offline-profile launch. This does not authenticate an account.
use crate::{LaunchPreparation, prepare_launch};
use cobble_auth::OfflineProfile;
use cobble_minecraft::{resolve_java, substitute};
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
    let plan = prepare_launch(version, directory).await?;
    let java = resolve_java(plan.required_java).await?;
    let arguments = offline_arguments(&plan, profile)?;
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

fn offline_arguments(
    plan: &LaunchPreparation,
    profile: &OfflineProfile,
) -> Result<Vec<String>, BoxError> {
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
    let mut arguments = Vec::new();
    if !plan.jvm_arguments.iter().any(|arg| arg.starts_with("-Xmx")) {
        arguments.push("-Xmx2048M".into());
    }
    for argument in &plan.jvm_arguments {
        arguments.push(bind(argument)?);
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
        let arguments = offline_arguments(&plan(), &profile).unwrap();
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
    fn preserves_metadata_heap_and_rejects_unbound_or_tampered_profile() {
        let profile = OfflineProfile::new("Notch").unwrap();
        let mut plan = plan();
        plan.jvm_arguments.push("-Xmx1024M".into());
        let arguments = offline_arguments(&plan, &profile).unwrap();
        assert!(!arguments.contains(&"-Xmx2048M".into()));
        plan.game_arguments.push("${clientid}".into());
        assert!(offline_arguments(&plan, &profile).is_err());
        plan.game_arguments.pop();
        plan.game_arguments.push("${unknown}".into());
        assert!(offline_arguments(&plan, &profile).is_err());
        let mut profile = profile;
        profile.uuid = "0".repeat(32);
        assert!(offline_arguments(&plan, &profile).is_err());
    }
}

// SPDX-License-Identifier: MIT
//! Host-owned offline or verified Microsoft identity launch.
use crate::{LaunchPreparation, prepare_launch};
use cobble_auth::{LaunchIdentity, OfflineProfile};
use cobble_minecraft::{
    LaunchBehavior, LaunchOverrides, WindowSize, read_launcher_preferences, resolve_java_override,
    substitute, validate_advanced_runtime, validate_memory_mb,
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
    pub launch_behavior: LaunchBehavior,
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
    let identity = LaunchIdentity::offline(profile)?;
    start_game_with_identity(version, directory, &identity, options).await
}

pub async fn start_game_with_identity(
    version: &str,
    directory: &Path,
    identity: &LaunchIdentity,
    options: &LaunchOverrides,
) -> Result<RunningGame, BoxError> {
    start_game_with_identity_and_server(version, directory, identity, options, None).await
}

pub async fn start_game_with_identity_and_server(
    version: &str,
    directory: &Path,
    identity: &LaunchIdentity,
    options: &LaunchOverrides,
    server: Option<&str>,
) -> Result<RunningGame, BoxError> {
    let server_arguments = server
        .map(|address| crate::server_launch::direct_connect_arguments(version, address))
        .transpose()?;
    let defaults = read_launcher_preferences()?;
    let memory_mb = options.memory_mb.unwrap_or(defaults.memory_mb);
    let window_size = options
        .window_size
        .as_ref()
        .unwrap_or(&defaults.window_size);
    let jvm_args = options.jvm_args.as_ref().unwrap_or(&defaults.jvm_args);
    validate_advanced_runtime(window_size, jvm_args)?;
    validate_memory_mb(memory_mb)?;
    let plan = prepare_launch(version, directory).await?;
    let java = resolve_java_override(plan.required_java, &options.java).await?;
    let mut arguments =
        prepare_identity_arguments_with_runtime(&plan, identity, memory_mb, window_size, jvm_args)?;
    if let Some(server_arguments) = server_arguments {
        arguments.extend(server_arguments);
    }
    let directory = dunce::canonicalize(directory)?;
    let logs = directory.join(".cobble-logs");
    fs::create_dir_all(&logs)?;
    let log_path = logs.join("launcher.log");
    let output = fs::File::create(&log_path)?;
    let mut command = Command::new(java.executable);
    command
        .arg(format!(
            "-Dcobble.launcher.session={}",
            plan.natives_directory.display()
        ))
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
        launch_behavior: options.launch_behavior.unwrap_or(defaults.launch_behavior),
    })
}

/// Host-only argv preparation with explicit runtime settings. Each extra JVM
/// option stays a single argument and is never interpreted by a shell.
pub fn prepare_identity_arguments_with_runtime(
    plan: &LaunchPreparation,
    identity: &LaunchIdentity,
    memory_mb: u32,
    window: &WindowSize,
    jvm_args: &[String],
) -> Result<Vec<String>, BoxError> {
    validate_advanced_runtime(window, jvm_args)?;
    let mut arguments = prepare_identity_arguments(plan, identity, memory_mb)?;
    let main_index = arguments.len() - plan.game_arguments.len() - 1;
    arguments.splice(main_index..main_index, jvm_args.iter().cloned());
    if let Some(width) = window.width {
        arguments.extend(["--width".into(), width.to_string()]);
    }
    if let Some(height) = window.height {
        arguments.extend(["--height".into(), height.to_string()]);
    }
    Ok(arguments)
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
    prepare_bound_arguments(
        plan,
        &profile.name,
        &profile.uuid,
        "0",
        "legacy",
        "",
        memory_mb,
    )
}

/// Host-only argv preparation. Do not expose the returned credentials in IPC or logs.
pub fn prepare_identity_arguments(
    plan: &LaunchPreparation,
    identity: &LaunchIdentity,
    memory_mb: u32,
) -> Result<Vec<String>, BoxError> {
    prepare_bound_arguments(
        plan,
        &identity.name,
        &identity.uuid,
        identity.access_token(),
        &identity.user_type,
        &identity.xuid,
        memory_mb,
    )
}

fn prepare_bound_arguments(
    plan: &LaunchPreparation,
    name: &str,
    uuid: &str,
    access_token: &str,
    user_type: &str,
    xuid: &str,
    memory_mb: u32,
) -> Result<Vec<String>, BoxError> {
    validate_memory_mb(memory_mb)?;
    let session = if user_type == "msa" {
        format!("token:{access_token}:{uuid}")
    } else {
        "0".into()
    };
    let placeholders = HashMap::from([
        ("auth_player_name".into(), name.to_owned()),
        ("auth_uuid".into(), uuid.to_owned()),
        ("auth_access_token".into(), access_token.to_owned()),
        ("auth_session".into(), session),
        ("user_type".into(), user_type.to_owned()),
        ("user_properties".into(), "{}".into()),
        ("auth_xuid".into(), xuid.to_owned()),
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

    #[test]
    fn extra_runtime_arguments_preserve_argv_and_window_dimensions() {
        let identity = LaunchIdentity::offline(&OfflineProfile::new("Notch").unwrap()).unwrap();
        let window = WindowSize {
            width: Some(1280),
            height: Some(720),
        };
        let arguments = prepare_identity_arguments_with_runtime(
            &plan(),
            &identity,
            4096,
            &window,
            &["-Dlabel=two words; $(ignored)".into()],
        )
        .unwrap();
        let main = arguments
            .iter()
            .position(|value| value == "net.minecraft.client.main.Main")
            .unwrap();
        assert_eq!(arguments[0], "-Xmx4096M");
        assert_eq!(arguments[main - 1], "-Dlabel=two words; $(ignored)");
        assert_eq!(
            &arguments[arguments.len() - 4..],
            ["--width", "1280", "--height", "720"]
        );
        assert!(
            prepare_identity_arguments_with_runtime(
                &plan(),
                &identity,
                4096,
                &window,
                &["-Xmx10G".into()]
            )
            .is_err()
        );
        assert_eq!(
            prepare_identity_arguments_with_runtime(
                &plan(),
                &identity,
                2048,
                &WindowSize::default(),
                &[]
            )
            .unwrap(),
            prepare_identity_arguments(&plan(), &identity, 2048).unwrap()
        );
    }

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

    #[test]
    fn online_identity_binds_minecraft_credentials_without_offline_uuid_derivation() {
        let arguments = prepare_bound_arguments(
            &plan(),
            "VerifiedPlayer",
            "0123456789abcdef0123456789abcdef",
            "synthetic-mc-token",
            "msa",
            "synthetic-xuid",
            2048,
        )
        .unwrap();
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--accessToken", "synthetic-mc-token"])
        );
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["--uuid", "0123456789abcdef0123456789abcdef"])
        );
        assert!(arguments.iter().any(|arg| arg == "msa"));
        assert!(arguments.iter().any(|arg| arg == "synthetic-xuid"));
        assert!(!arguments.iter().any(|arg| arg.contains("${")));
    }
}

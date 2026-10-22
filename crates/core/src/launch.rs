// SPDX-License-Identifier: MIT
//! Offline preparation of an installed modern Vanilla game. No authentication or spawn.
use cobble_minecraft::{
    FileKind, PlatformContext, VersionMetadata, native_artifact_matches, resolve_arguments,
    substitute,
};
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Error = Box<dyn std::error::Error + Send + Sync>;

/// Contains no account secrets. Authentication placeholders must be bound by a
/// process runner; this is not an executable launch command.
#[derive(Debug, Serialize)]
pub struct LaunchPreparation {
    pub required_java: u32,
    pub main_class: String,
    pub classpath_entries: usize,
    pub classpath: Vec<PathBuf>,
    pub jvm_arguments: Vec<String>,
    pub game_arguments: Vec<String>,
    pub natives_directory: PathBuf,
    pub native_files: usize,
}

pub async fn prepare_launch(version: &str, directory: &Path) -> Result<LaunchPreparation, Error> {
    let version = version.to_owned();
    let directory = directory.to_owned();
    tokio::task::spawn_blocking(move || prepare(&version, &directory)).await?
}

#[derive(Deserialize)]
struct InstallState {
    version: String,
    complete: bool,
    client_only: bool,
}

fn prepare(version: &str, directory: &Path) -> Result<LaunchPreparation, Error> {
    SafeRelativePath::parse(version)?;
    if version.contains('/') {
        return Err("invalid version identifier".into());
    }
    let directory = dunce::canonicalize(directory)?;
    crate::foreign_import::ensure_import_complete(&directory)?;
    crate::mrpack::ensure_pack_complete(&directory)?;
    let state: InstallState =
        serde_json::from_reader(File::open(directory.join(".cobble-install.json"))?)?;
    if state.version != version || !state.complete || state.client_only {
        return Err("selected installation is incomplete or belongs to a different version".into());
    }
    let bytes = fs::read(directory.join(format!("versions/{version}/{version}.json")))?;
    let metadata = VersionMetadata::parse_installed(&bytes, version)?;
    let fabric = crate::fabric::load_overlay(&directory, version)?;
    let forge = crate::forge::load_overlay(&directory, version)?;
    if fabric.is_some() && forge.is_some() {
        return Err("multiple loader overlays exist in this instance".into());
    }
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    // Keep classifier and extension in the identity: generated client and
    // universal JARs for the same loader must both remain on the classpath.
    let mut overridden = std::collections::HashSet::new();
    if let Some(state) = &fabric {
        for library in &state.profile.libraries {
            // Fabric's validated profile supports unclassified JARs only.
            overridden.insert(format!("{}:@jar", library.identity()));
        }
    }
    if let Some(state) = &forge {
        for library in &state.profile.libraries {
            overridden.insert(library.identity()?);
        }
    }
    let replaced_paths: std::collections::HashSet<&str> = raw["libraries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|library| {
            let name = library["name"].as_str()?;
            let identity = crate::forge::maven_identity(name).ok()?;
            overridden
                .contains(&identity)
                .then(|| library["downloads"]["artifact"]["path"].as_str())
                .flatten()
        })
        .collect();
    let platform = PlatformContext::current()?;
    let plan = metadata.download_plan(&platform)?;
    let has_native_artifacts = plan
        .files
        .iter()
        .any(|file| file.kind == FileKind::Native || file.path.as_str().contains("-natives-"));
    let mut classpath = match &fabric {
        Some(state) => crate::fabric::overlay_classpath(state, &directory)?,
        None => match &forge {
            Some(state) => crate::forge::overlay_classpath(state, &directory)?,
            None => Vec::new(),
        },
    };
    let mut native_jars = Vec::new();
    let mut client = None;
    for file in &plan.files {
        if file.kind == FileKind::Library
            && replaced_paths.contains(file.path.as_str().trim_start_matches("libraries/"))
        {
            continue;
        }
        if !native_artifact_matches(file.path.as_str(), &platform) {
            continue;
        }
        let path = directory.join(file.path.as_str());
        if !path.is_file() {
            return Err(format!("missing installed file: {}", file.path.as_str()).into());
        }
        match file.kind {
            FileKind::Client => client = Some(path),
            FileKind::Library => {
                if file.path.as_str().contains("-natives-") {
                    native_jars.push(path.clone());
                }
                classpath.push(path);
            }
            FileKind::Native => native_jars.push(path),
            _ => (),
        }
    }
    if has_native_artifacts && native_jars.is_empty() {
        return Err("unsupported native libraries for this JVM architecture".into());
    }
    classpath.push(client.ok_or("missing client artifact")?);
    let natives_root = directory.join(".cobble-natives");
    fs::create_dir_all(&natives_root)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let natives_directory = natives_root.join(format!("{}-{nonce}", std::process::id()));
    fs::create_dir(&natives_directory)?;
    let mut native_files = 0;
    for jar in native_jars {
        native_files += extract_natives(&jar, &natives_directory, &platform.os)?;
    }
    let path_string = |path: &Path| -> Result<String, Error> {
        Ok(path
            .to_str()
            .ok_or("game path is not valid Unicode")?
            .to_owned())
    };
    let mut placeholders = HashMap::from([
        ("clientid".into(), cobble_auth::MICROSOFT_CLIENT_ID.into()),
        ("version_name".into(), version.into()),
        (
            "version_type".into(),
            raw["type"].as_str().ok_or("missing version type")?.into(),
        ),
        ("game_directory".into(), path_string(&directory)?),
        (
            "assets_root".into(),
            path_string(&directory.join("assets"))?,
        ),
        ("assets_index_name".into(), metadata.asset_index.id.clone()),
        ("natives_directory".into(), path_string(&natives_directory)?),
        (
            "library_directory".into(),
            path_string(&directory.join("libraries"))?,
        ),
        ("launcher_name".into(), "CobbleLauncher".into()),
        ("launcher_version".into(), env!("CARGO_PKG_VERSION").into()),
        (
            "classpath_separator".into(),
            if platform.os == "windows" { ";" } else { ":" }.into(),
        ),
        (
            "classpath".into(),
            std::env::join_paths(&classpath)?
                .into_string()
                .map_err(|_| "classpath is not valid Unicode")?,
        ),
    ]);
    let mut jvm_arguments =
        resolve_arguments(&metadata.arguments["jvm"], &platform, &placeholders)?;
    if let Some((argument, file)) = metadata.logging_argument() {
        placeholders.insert(
            "path".into(),
            path_string(&directory.join("assets/log_configs").join(file))?,
        );
        jvm_arguments.push(substitute(argument, &placeholders)?);
    }
    let overlay = fabric
        .as_ref()
        .map(|state| {
            (
                &state.profile.id,
                &state.profile.main_class,
                &state.profile.arguments,
            )
        })
        .or_else(|| {
            forge.as_ref().map(|state| {
                (
                    &state.profile.id,
                    &state.profile.main_class,
                    &state.profile.arguments,
                )
            })
        });
    if let Some((_, _, arguments)) = overlay {
        jvm_arguments.extend(resolve_arguments(
            &arguments["jvm"],
            &platform,
            &placeholders,
        )?);
    }
    // JVM version_name remains the base version for NeoForge's ignoreList.
    // Game version_name identifies the selected child profile.
    if let Some((id, _, _)) = overlay {
        placeholders.insert("version_name".into(), id.clone());
    }
    let mut game_arguments =
        resolve_arguments(&metadata.arguments["game"], &platform, &placeholders)?;
    let mut main_class = metadata.main_class;
    if let Some((_, overlay_main, arguments)) = overlay {
        main_class = overlay_main.clone();
        game_arguments.extend(resolve_arguments(
            &arguments["game"],
            &platform,
            &placeholders,
        )?);
    }
    Ok(LaunchPreparation {
        required_java: metadata.java_version.major_version,
        main_class,
        classpath_entries: classpath.len(),
        classpath,
        jvm_arguments,
        game_arguments,
        natives_directory,
        native_files,
    })
}

fn extract_natives(jar: &Path, destination: &Path, os: &str) -> Result<usize, Error> {
    let mut archive = zip::ZipArchive::new(File::open(jar)?)?;
    let mut count = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(path) = entry.enclosed_name() else {
            return Err("unsafe native archive path".into());
        };
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let binary = match os {
            "windows" => name.ends_with(".dll"),
            "osx" => name.ends_with(".dylib"),
            _ => name.ends_with(".so"),
        };
        if !binary || entry.is_dir() {
            continue;
        }
        SafeRelativePath::parse(name)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("native archive contains a symlink".into());
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination.join(name))?;
        io::copy(&mut entry, &mut output)?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fabric_preparation_merges_arguments_and_replaces_parent_library_offline() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cobble-fabric-launch-{nonce}"));
        fs::create_dir_all(root.join("versions/synthetic")).unwrap();
        fs::create_dir_all(root.join("assets/indexes")).unwrap();
        fs::create_dir_all(root.join("libraries/example/asm/2")).unwrap();
        let jar = root.join("libraries/example/asm/2/asm-2.jar");
        fs::write(&jar, b"synthetic library").unwrap();
        fs::write(
            root.join("versions/synthetic/synthetic.jar"),
            b"synthetic client",
        )
        .unwrap();
        fs::write(root.join("assets/indexes/synthetic.json"), b"{}").unwrap();
        fs::write(
            root.join(".cobble-install.json"),
            br#"{"version":"synthetic","complete":true,"client_only":false}"#,
        )
        .unwrap();
        let artifact = serde_json::json!({"url":"https://example.invalid/unused", "sha1":"0000000000000000000000000000000000000000", "size":1});
        let mut old = artifact.clone();
        old["path"] = "example/asm/1/asm-1.jar".into();
        let metadata = serde_json::json!({
            "id":"synthetic", "type":"release", "mainClass":"example.Vanilla", "javaVersion":{"majorVersion":21},
            "assetIndex":{"id":"synthetic","url":"https://example.invalid/index", "sha1":"0000000000000000000000000000000000000000","size":2},
            "downloads":{"client":artifact}, "libraries":[{"name":"example:asm:1", "downloads":{"artifact":old}}],
            "arguments":{"jvm":["-cp","${classpath}"], "game":["--username","${auth_player_name}"]}
        });
        fs::write(
            root.join("versions/synthetic/synthetic.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let overlay = serde_json::json!({
            "mc_version":"synthetic","loader_version":"0.16.14","complete":true,
            "profile":{"id":"fabric-loader-0.16.14-synthetic","inheritsFrom":"synthetic","mainClass":"net.fabricmc.loader.impl.launch.knot.KnotClient",
                "arguments":{"jvm":["-Dfixture=one argument with spaces"],"game":["--fixture"]},
                "libraries":[{"name":"example:asm:2","url":"https://maven.fabricmc.net/"}]}
        });
        fs::write(
            root.join(".cobble-fabric.json"),
            serde_json::to_vec(&overlay).unwrap(),
        )
        .unwrap();
        let plan = prepare("synthetic", &root).unwrap();
        assert_eq!(
            plan.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(plan.classpath.len(), 2);
        assert!(plan.classpath[0].ends_with("asm-2.jar"));
        assert_eq!(
            plan.jvm_arguments.last().unwrap(),
            "-Dfixture=one argument with spaces"
        );
        assert_eq!(
            plan.game_arguments,
            ["--username", "${auth_player_name}", "--fixture"]
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn neoforge_preparation_preserves_module_args_and_separates_version_placeholders() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cobble-neoforge-launch-{nonce}"));
        fs::create_dir_all(root.join("versions/1.21.1")).unwrap();
        fs::create_dir_all(root.join("assets/indexes")).unwrap();
        fs::write(root.join("versions/1.21.1/1.21.1.jar"), b"synthetic client").unwrap();
        fs::write(root.join("assets/indexes/synthetic.json"), b"{}").unwrap();
        fs::write(
            root.join(".cobble-install.json"),
            br#"{"version":"1.21.1","complete":true,"client_only":false}"#,
        )
        .unwrap();
        let artifact = serde_json::json!({"url":"https://example.invalid/unused", "sha1":"0".repeat(40), "size":1});
        let mut old = artifact.clone();
        old["path"] = "example/asm/1/asm-1.jar".into();
        let metadata = serde_json::json!({
            "id":"1.21.1", "type":"release", "mainClass":"example.Vanilla", "javaVersion":{"majorVersion":21},
            "assetIndex":{"id":"synthetic","url":"https://example.invalid/index", "sha1":"0".repeat(40),"size":2},
            "downloads":{"client":artifact}, "libraries":[{"name":"example:asm:1", "downloads":{"artifact":old}}],
            "arguments":{"jvm":["-cp","${classpath}"], "game":["--version","${version_name}"]}
        });
        fs::write(
            root.join("versions/1.21.1/1.21.1.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let mut libraries = Vec::new();
        for (name, path) in [
            ("example:asm:2", "example/asm/2/asm-2.jar"),
            (
                "example:loader:1:client",
                "example/loader/1/loader-1-client.jar",
            ),
            (
                "example:loader:1:universal",
                "example/loader/1/loader-1-universal.jar",
            ),
        ] {
            let disk = root.join("libraries").join(path);
            fs::create_dir_all(disk.parent().unwrap()).unwrap();
            fs::write(disk, b"synthetic library").unwrap();
            libraries.push(serde_json::json!({"name":name,"downloads":{"artifact":{"path":path,"sha1":"0".repeat(40),"size":1,"url":""}}}));
        }
        let overlay = serde_json::json!({
            "kind":"neoforge","mc_version":"1.21.1","loader_version":"21.1.264","complete":true,
            "profile":{"id":"neoforge-21.1.264","inheritsFrom":"1.21.1","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher",
                "arguments":{"jvm":["-DignoreList=client-extra,${version_name}.jar","-p","${library_directory}/example/loader/1/loader-1-universal.jar","--add-modules","ALL-MODULE-PATH"],"game":["--launchTarget","forgeclient"]},
                "libraries":libraries}
        });
        fs::write(
            root.join(".cobble-forge.json"),
            serde_json::to_vec(&overlay).unwrap(),
        )
        .unwrap();
        let plan = prepare("1.21.1", &root).unwrap();
        assert_eq!(
            plan.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        assert_eq!(plan.classpath.len(), 4);
        assert!(plan.classpath[0].ends_with("asm-2.jar"));
        assert!(
            plan.classpath
                .iter()
                .any(|path| path.ends_with("loader-1-client.jar"))
        );
        assert!(
            plan.classpath
                .iter()
                .any(|path| path.ends_with("loader-1-universal.jar"))
        );
        assert!(
            plan.jvm_arguments
                .iter()
                .any(|argument| argument == "-DignoreList=client-extra,1.21.1.jar")
        );
        assert!(
            plan.jvm_arguments
                .windows(2)
                .any(|pair| pair == ["--add-modules", "ALL-MODULE-PATH"])
        );
        let module = plan
            .jvm_arguments
            .iter()
            .position(|argument| argument == "-p")
            .unwrap();
        assert!(
            plan.jvm_arguments[module + 1].ends_with("example/loader/1/loader-1-universal.jar")
        );
        assert_eq!(
            plan.game_arguments,
            [
                "--version",
                "neoforge-21.1.264",
                "--launchTarget",
                "forgeclient"
            ]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_jars_extract_only_binaries_and_do_not_replace_existing_files() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("cobble-native-test-{nonce}"));
        fs::create_dir(&root).unwrap();
        let jar = root.join("fixture.jar");
        let mut zip = zip::ZipWriter::new(File::create(&jar).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
        zip.write_all(b"original fixture").unwrap();
        zip.start_file("fixture.dll", options).unwrap();
        zip.write_all(b"synthetic native bytes").unwrap();
        zip.finish().unwrap();
        let destination = root.join("natives");
        fs::create_dir(&destination).unwrap();
        assert_eq!(extract_natives(&jar, &destination, "windows").unwrap(), 1);
        assert_eq!(
            fs::read(destination.join("fixture.dll")).unwrap(),
            b"synthetic native bytes"
        );
        assert!(!destination.join("META-INF").exists());
        assert!(extract_natives(&jar, &destination, "windows").is_err());
        fs::remove_file(destination.join("fixture.dll")).unwrap();
        fs::remove_dir(destination).unwrap();
        fs::remove_file(jar).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

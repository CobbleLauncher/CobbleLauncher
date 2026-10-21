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
/// future authenticated process runner; this is not an executable launch command.
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
    let directory = fs::canonicalize(directory)?;
    let state: InstallState =
        serde_json::from_reader(File::open(directory.join(".cobble-install.json"))?)?;
    if state.version != version || !state.complete || state.client_only {
        return Err("selected installation is incomplete or belongs to a different version".into());
    }
    let bytes = fs::read(directory.join(format!("versions/{version}/{version}.json")))?;
    let metadata = VersionMetadata::parse_installed(&bytes, version)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
    let platform = PlatformContext::current()?;
    let plan = metadata.download_plan(&platform)?;
    let has_native_artifacts = plan
        .files
        .iter()
        .any(|file| file.kind == FileKind::Native || file.path.as_str().contains("-natives-"));
    let mut classpath = Vec::new();
    let mut native_jars = Vec::new();
    let mut client = None;
    for file in &plan.files {
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
    let game_arguments = resolve_arguments(&metadata.arguments["game"], &platform, &placeholders)?;
    Ok(LaunchPreparation {
        required_java: metadata.java_version.major_version,
        main_class: metadata.main_class,
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

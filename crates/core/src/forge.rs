// SPDX-License-Identifier: MIT
//! Modern Forge-family overlays, using the upstream Java installer itself.
//! Independently implemented from public profiles and the MIT XMCL behavior
//! reference (154e2719b1ac962631d80ae4a280382f4f1ac09d); no source copied.
use crate::{InstallOutcome, InstallProgress};
use cobble_minecraft::{DownloadSource, PublicFetchError};
use cobble_net::{Broker, BrokerConfig, NetError};
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
const MARKER: &str = ".cobble-forge.json";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ForgeKind {
    Forge,
    NeoForge,
}

impl ForgeKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }
    fn installer_url(self, mc: &str, version: &str) -> String {
        match self {
            Self::Forge => format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{version}/forge-{mc}-{version}-installer.jar"
            ),
            Self::NeoForge => format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ForgeVersion {
    pub version: String,
    pub recommended: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForgeState {
    pub kind: ForgeKind,
    pub mc_version: String,
    pub loader_version: String,
    pub complete: bool,
    pub profile: ForgeProfile,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ForgeProfile {
    pub id: String,
    pub inherits_from: String,
    pub main_class: String,
    pub arguments: serde_json::Value,
    pub libraries: Vec<ForgeLibrary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForgeLibrary {
    pub name: String,
    pub downloads: ForgeDownloads,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForgeDownloads {
    pub artifact: ForgeArtifact,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForgeArtifact {
    pub path: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

pub async fn list_forge_versions(mc: &str, kind: ForgeKind) -> Result<Vec<ForgeVersion>, Error> {
    let source = cobble_minecraft::read_download_settings()?.download_source()?;
    list_forge_versions_from(mc, kind, &source).await
}

pub async fn list_forge_versions_from(
    mc: &str,
    kind: ForgeKind,
    source: &DownloadSource,
) -> Result<Vec<ForgeVersion>, Error> {
    supported(mc, kind, None)?;
    let broker = Broker::new(BrokerConfig::default())?;
    match kind {
        ForgeKind::Forge => {
            fetch_metadata(
                source,
                &broker,
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
                1024 * 1024,
                |bytes| parse_forge_versions(bytes, mc),
            )
            .await
        }
        ForgeKind::NeoForge => {
            fetch_metadata(
                source,
                &broker,
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
                4 * 1024 * 1024,
                parse_neoforge_versions,
            )
            .await
        }
    }
}

fn parse_forge_versions(bytes: &[u8], mc: &str) -> Result<Vec<ForgeVersion>, Error> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if !value["promos"].is_object() {
        return Err("missing Forge promotions".into());
    }
    let mut versions = Vec::new();
    for (suffix, recommended) in [("recommended", true), ("latest", false)] {
        if let Some(version) = value["promos"][format!("{mc}-{suffix}")].as_str()
            && !versions
                .iter()
                .any(|item: &ForgeVersion| item.version == version)
        {
            supported(mc, ForgeKind::Forge, Some(version))?;
            versions.push(ForgeVersion {
                version: version.into(),
                recommended,
            });
        }
    }
    Ok(versions)
}

fn parse_neoforge_versions(bytes: &[u8]) -> Result<Vec<ForgeVersion>, Error> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let mut versions: Vec<_> = value["versions"]
        .as_array()
        .ok_or("missing NeoForge versions")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .filter(|version| {
            version.starts_with("21.1.")
                && version.split('.').all(|part| part.parse::<u32>().is_ok())
        })
        .map(|version| ForgeVersion {
            version: version.into(),
            recommended: false,
        })
        .collect();
    versions.sort_by_key(|version| {
        version
            .version
            .rsplit('.')
            .next()
            .and_then(|part| part.parse::<u32>().ok())
    });
    versions.reverse();
    versions.truncate(20);
    Ok(versions)
}

/// The caller owns the stopped-instance write lease and explicitly chose this
/// loader. Executes only its official installer; this is ordinary native code,
/// not a sandbox. Installer library downloads currently use upstream sources.
pub async fn install_forge(
    mc: &str,
    version: &str,
    kind: ForgeKind,
    target: &Path,
    source: DownloadSource,
    on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync>,
) -> Result<InstallOutcome, Error> {
    install_forge_controlled(
        mc,
        version,
        kind,
        target,
        source,
        crate::OperationControl::new(on_progress),
    )
    .await
}

pub async fn install_forge_controlled(
    mc: &str,
    version: &str,
    kind: ForgeKind,
    target: &Path,
    source: DownloadSource,
    control: crate::OperationControl,
) -> Result<InstallOutcome, Error> {
    control.check_cancelled()?;
    let on_progress = control.progress();

    supported(mc, kind, Some(version))?;
    if target.join(".cobble-fabric.json").exists() {
        return Err("this instance already has Fabric".into());
    }
    if let Some(old) = load_state(target)? {
        if old.kind != kind || old.mc_version != mc || old.loader_version != version {
            return Err("this instance belongs to another loader installation".into());
        }
        if old.complete {
            validate_profile(&old.profile, mc, version, kind)?;
            if completed_install_ready(target, mc, &old.profile) {
                return Ok(InstallOutcome {
                    version: mc.into(),
                    java_major: 21,
                });
            }
        }
    }
    let java = cobble_minecraft::resolve_java(21).await?;
    control.check_cancelled()?;
    let outcome =
        crate::install_game_controlled(mc, target, source.clone(), control.clone()).await?;
    let directory = dunce::canonicalize(target)?;
    let broker = Broker::new(BrokerConfig {
        total_timeout: Duration::from_secs(300),
        ..BrokerConfig::default()
    })?;
    let url = kind.installer_url(mc, version);
    let checksum = fetch(&broker, &format!("{url}.sha1"), 256).await?;
    let expected = parse_sha1(std::str::from_utf8(&checksum)?.trim())?;
    // Binaries follow the remembered metadata winner, without racing JAR bodies.
    // Keep the loader-only system-proxy fallback on every concrete attempt.
    let mut bytes = None;
    let mut last_error = None;
    for (candidate, download_url) in source.candidates(&url)? {
        control.check_cancelled()?;
        match fetch(&broker, &download_url, 32 * 1024 * 1024).await {
            Ok(value) => {
                bytes = Some(value);
                break;
            }
            Err(error) => {
                candidate.report_failure();
                last_error = Some(error);
            }
        }
    }
    let bytes = bytes
        .ok_or_else(|| last_error.unwrap_or_else(|| "no installer download sources".into()))?;
    if Sha1::digest(&bytes).as_slice() != expected {
        return Err("official installer checksum mismatch".into());
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes))?;
    let install: serde_json::Value = serde_json::from_reader(
        archive
            .by_name("install_profile.json")?
            .take(4 * 1024 * 1024),
    )?;
    if install["spec"] != 1
        || install["minecraft"].as_str() != Some(mc)
        || install["json"].as_str() != Some("/version.json")
    {
        return Err("unsupported official installer profile".into());
    }
    let mut profile: ForgeProfile =
        serde_json::from_reader(archive.by_name("version.json")?.take(4 * 1024 * 1024))?;
    validate_profile(&profile, mc, version, kind)?;
    normalize_libraries(&mut profile)?;
    let mut state = ForgeState {
        kind,
        mc_version: mc.into(),
        loader_version: version.into(),
        complete: false,
        profile,
    };
    write_state(&directory, &state)?;
    let installers = directory.join(".cobble-installers");
    fs::create_dir_all(&installers)?;
    let installer = installers.join(format!("{}-{version}-installer.jar", kind.id()));
    fs::write(&installer, &bytes)?;
    let launcher = directory.join("launcher_profiles.json");
    if !launcher.exists() {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(launcher)?
            .write_all(br#"{"profiles":{}}"#)?;
    }
    let log = directory.join(".cobble-forge-install.log");
    let output = File::create(&log)?;
    on_progress(InstallProgress {
        phase: format!("{}-installer", kind.id()),
        completed: 0,
        total: 1,
        reused: 0,
    });
    let mut command = tokio::process::Command::new(java.executable);
    command
        .arg("-Djava.awt.headless=true")
        .arg("-Djava.net.useSystemProxies=true")
        .arg("-jar")
        .arg(&installer)
        .arg(match kind {
            ForgeKind::Forge => "--installClient",
            ForgeKind::NeoForge => "--install-client",
        })
        .arg(&directory)
        .current_dir(&directory)
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdin(Stdio::null())
        .stdout(Stdio::from(output.try_clone()?))
        .stderr(Stdio::from(output))
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    control.check_cancelled()?;
    let mut child = command.spawn()?;
    let status = wait_installer(&mut child, &control, &log).await?;
    if !status.success() {
        return Err(format!("installer exited with {status}; see {}", log.display()).into());
    }
    let installed_path = directory.join(format!("versions/{0}/{0}.json", state.profile.id));
    let mut installed: ForgeProfile =
        serde_json::from_reader(File::open(installed_path)?.take(4 * 1024 * 1024))?;
    validate_profile(&installed, mc, version, kind)?;
    normalize_libraries(&mut installed)?;
    state.profile = installed;
    verify_libraries(&directory, &state.profile)?;
    verify_outputs(&directory, &install, kind)?;
    control.check_cancelled()?;
    state.complete = true;
    write_state(&directory, &state)?;
    on_progress(InstallProgress {
        phase: "complete".into(),
        completed: 1,
        total: 1,
        reused: 0,
    });
    Ok(outcome)
}

async fn wait_installer(
    child: &mut tokio::process::Child,
    control: &crate::OperationControl,
    log: &Path,
) -> Result<std::process::ExitStatus, Error> {
    let result = tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(900), child.wait()) => Some(result),
        _ = control.cancelled() => None,
    };
    match result {
        None => {
            let _ = child.start_kill();
            child.wait().await?;
            Err(crate::OperationCancelled.into())
        }
        Some(Ok(result)) => Ok(result?),
        Some(Err(_)) => {
            let _ = child.start_kill();
            child.wait().await?;
            Err(format!("installer timed out; see {}", log.display()).into())
        }
    }
}

fn supported(mc: &str, kind: ForgeKind, version: Option<&str>) -> Result<(), Error> {
    if mc != "1.21.1" {
        return Err("Forge adapters currently support only Minecraft 1.21.1".into());
    }
    if let Some(version) = version {
        let prefix = match kind {
            ForgeKind::Forge => "52.",
            ForgeKind::NeoForge => "21.1.",
        };
        if !version.starts_with(prefix)
            || !version.split('.').all(|part| part.parse::<u32>().is_ok())
        {
            return Err("unsupported Forge-family loader version".into());
        }
    }
    Ok(())
}

fn validate_profile(
    profile: &ForgeProfile,
    mc: &str,
    version: &str,
    kind: ForgeKind,
) -> Result<(), Error> {
    let expected_id = match kind {
        ForgeKind::Forge => format!("{mc}-forge-{version}"),
        ForgeKind::NeoForge => format!("neoforge-{version}"),
    };
    let main = match kind {
        ForgeKind::Forge => "net.minecraftforge.bootstrap.ForgeBootstrap",
        ForgeKind::NeoForge => "cpw.mods.bootstraplauncher.BootstrapLauncher",
    };
    if profile.id != expected_id
        || profile.inherits_from != mc
        || profile.main_class != main
        || profile.libraries.is_empty()
        || profile.libraries.len() > 256
    {
        return Err("unsupported or mismatched Forge-family client profile".into());
    }
    for library in &profile.libraries {
        library.path()?;
        parse_sha1(&library.downloads.artifact.sha1)?;
    }
    if !profile.arguments["game"].is_array() || !profile.arguments["jvm"].is_array() {
        return Err("unsupported loader argument format".into());
    }
    Ok(())
}

fn normalize_libraries(profile: &mut ForgeProfile) -> Result<(), Error> {
    // Official Forge profiles repeat some runtime libraries. The final child
    // entry wins while classifier-distinct client/universal artifacts remain.
    let mut identities = BTreeSet::new();
    let mut libraries = Vec::new();
    for library in std::mem::take(&mut profile.libraries).into_iter().rev() {
        if identities.insert(library.identity()?) {
            libraries.push(library);
        }
    }
    libraries.reverse();
    profile.libraries = libraries;
    Ok(())
}

impl ForgeLibrary {
    pub(crate) fn identity(&self) -> Result<String, Error> {
        maven_identity(&self.name)
    }
    pub(crate) fn path(&self) -> Result<SafeRelativePath, Error> {
        let expected = maven_path(&self.name)?;
        if expected != self.downloads.artifact.path {
            return Err("loader Maven coordinate/path mismatch".into());
        }
        Ok(SafeRelativePath::parse(&format!("libraries/{expected}"))?)
    }
}

pub(crate) fn maven_identity(name: &str) -> Result<String, Error> {
    let (coordinate, extension) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<_> = coordinate.split(':').collect();
    if !(3..=4).contains(&parts.len())
        || parts.iter().chain(std::iter::once(&extension)).any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-+".contains(&byte))
        })
    {
        return Err("unsupported loader Maven coordinate".into());
    }
    Ok(format!(
        "{}:{}:{}@{extension}",
        parts[0],
        parts[1],
        parts.get(3).copied().unwrap_or("")
    ))
}

fn maven_path(name: &str) -> Result<String, Error> {
    maven_identity(name)?;
    let (coordinate, extension) = name.split_once('@').unwrap_or((name, "jar"));
    let parts: Vec<_> = coordinate.split(':').collect();
    let classifier = parts
        .get(3)
        .map(|value| format!("-{value}"))
        .unwrap_or_default();
    let path = format!(
        "{}/{}/{}/{}-{}{classifier}.{extension}",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    );
    SafeRelativePath::parse(&path)?;
    Ok(path)
}

fn verify_libraries(directory: &Path, profile: &ForgeProfile) -> Result<(), Error> {
    for library in &profile.libraries {
        installed_file(&directory.join(library.path()?.as_str()))?;
    }
    Ok(())
}

fn completed_install_ready(directory: &Path, mc: &str, profile: &ForgeProfile) -> bool {
    let base: Option<serde_json::Value> = File::open(directory.join(".cobble-install.json"))
        .ok()
        .and_then(|file| serde_json::from_reader(file.take(1024 * 1024)).ok());
    let Some(base) = base else {
        return false;
    };
    base["version"].as_str() == Some(mc)
        && base["complete"].as_bool() == Some(true)
        && base["client_only"].as_bool() == Some(false)
        && directory.join(format!("versions/{mc}/{mc}.json")).is_file()
        && directory.join(format!("versions/{mc}/{mc}.jar")).is_file()
        && verify_libraries(directory, profile).is_ok()
}

fn verify_outputs(
    directory: &Path,
    install: &serde_json::Value,
    kind: ForgeKind,
) -> Result<(), Error> {
    let keys: &[&str] = match kind {
        ForgeKind::Forge => &["PATCHED"],
        ForgeKind::NeoForge => &["MC_EXTRA", "MC_SRG", "PATCHED"],
    };
    for key in keys {
        let coordinate = install["data"][*key]["client"]
            .as_str()
            .ok_or("missing installer client output")?;
        let coordinate = coordinate
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .ok_or("unsupported installer output location")?;
        installed_file(&directory.join("libraries").join(maven_path(coordinate)?))?;
    }
    Ok(())
}

fn installed_file(path: &Path) -> Result<(), Error> {
    if !path.is_file() {
        return Err(format!("missing installed loader file: {}", path.display()).into());
    }
    Ok(())
}

pub(crate) fn load_overlay(directory: &Path, mc: &str) -> Result<Option<ForgeState>, Error> {
    let Some(state) = load_state(directory)? else {
        return Ok(None);
    };
    if !state.complete || state.mc_version != mc {
        return Err(
            "Forge-family installation is incomplete or belongs to another Minecraft version"
                .into(),
        );
    }
    validate_profile(&state.profile, mc, &state.loader_version, state.kind)?;
    Ok(Some(state))
}

pub(crate) fn overlay_classpath(
    state: &ForgeState,
    directory: &Path,
) -> Result<Vec<PathBuf>, Error> {
    state
        .profile
        .libraries
        .iter()
        .map(|library| {
            let path = directory.join(library.path()?.as_str());
            if !path.is_file() {
                return Err(format!("missing loader library {}", library.name).into());
            }
            Ok(path)
        })
        .collect()
}

fn load_state(directory: &Path) -> Result<Option<ForgeState>, Error> {
    match File::open(directory.join(MARKER)) {
        Ok(file) => {
            let mut state: ForgeState = serde_json::from_reader(file.take(4 * 1024 * 1024))?;
            normalize_libraries(&mut state.profile)?;
            Ok(Some(state))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn write_state(directory: &Path, state: &ForgeState) -> Result<(), Error> {
    let temporary = directory.join(".cobble-forge.json.tmp");
    let mut file = File::create(&temporary)?;
    serde_json::to_writer_pretty(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, directory.join(MARKER))?;
    Ok(())
}

fn parse_sha1(value: &str) -> Result<[u8; 20], Error> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid upstream SHA-1".into());
    }
    let mut digest = [0; 20];
    for (number, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[number * 2..number * 2 + 2], 16)?;
    }
    Ok(digest)
}

fn proxy_retryable(error: &Error) -> bool {
    matches!(
        error.downcast_ref::<PublicFetchError>(),
        Some(PublicFetchError::Network(
            NetError::Connect | NetError::Tls | NetError::Timeout(_)
        ))
    )
}

async fn fetch_metadata<T, F>(
    source: &DownloadSource,
    broker: &Broker,
    url: &str,
    limit: u64,
    parse: F,
) -> Result<T, Error>
where
    F: Fn(&[u8]) -> Result<T, Error> + Sync,
{
    match source
        .fetch_public_validated(broker, url, limit, &parse)
        .await
    {
        Err(error) if proxy_retryable(&error) => {
            let bytes = cobble_net::fetch_loader_via_system_proxy(url, limit).await?;
            parse(&bytes)
        }
        result => result,
    }
}

async fn fetch(broker: &Broker, url: &str, limit: u64) -> Result<Vec<u8>, Error> {
    let response = match broker.get(url, None, limit).await {
        Ok(response) => response,
        Err(NetError::Connect | NetError::Tls | NetError::Timeout(_)) => {
            return Ok(cobble_net::fetch_loader_via_system_proxy(url, limit).await?);
        }
        Err(error) => return Err(error.into()),
    };
    if response.metadata.status != 200 {
        return Err(format!(
            "loader service returned HTTP {} for {url}",
            response.metadata.status
        )
        .into());
    }
    Ok(response.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_installer_fixture() {
        if std::env::var_os("COBBLE_TEST_INSTALLER_WAIT").is_some() {
            std::thread::sleep(Duration::from_secs(10));
        }
    }

    #[test]
    fn cancelling_owned_installer_waits_until_its_process_has_exited() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "forge::tests::owned_installer_fixture"])
                .env("COBBLE_TEST_INSTALLER_WAIT", "1")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            #[cfg(windows)]
            command.creation_flags(0x0800_0000);
            let mut child = command.spawn().unwrap();
            assert!(child.id().is_some());
            let control = crate::OperationControl::new(Arc::new(|_| {}));
            control.cancel();
            let result = wait_installer(&mut child, &control, Path::new("fixture.log")).await;
            assert!(result.unwrap_err().is::<crate::OperationCancelled>());
            assert!(child.try_wait().unwrap().is_some());
        });
    }

    #[test]
    fn loader_metadata_validation_rejects_invalid_success_bodies() {
        assert!(parse_forge_versions(b"{}", "1.21.1").is_err());
        assert!(parse_forge_versions(b"<html>proxy error</html>", "1.21.1").is_err());
        let forge = parse_forge_versions(
            br#"{"promos":{"1.21.1-recommended":"52.1.0","1.21.1-latest":"52.1.0"}}"#,
            "1.21.1",
        )
        .unwrap();
        assert_eq!(forge.len(), 1);
        assert!(forge[0].recommended);
        assert!(parse_neoforge_versions(b"{}").is_err());
        let neo =
            parse_neoforge_versions(br#"{"versions":["21.1.9","21.2.1","21.1.10"]}"#).unwrap();
        assert_eq!(
            neo.iter()
                .map(|version| version.version.as_str())
                .collect::<Vec<_>>(),
            ["21.1.10", "21.1.9"]
        );
    }

    #[test]
    fn proxy_fallback_stays_limited_to_connection_tls_and_timeout_errors() {
        for error in [
            NetError::Connect,
            NetError::Tls,
            NetError::Timeout(cobble_net::TimeoutPhase::Total),
        ] {
            let error: Error = PublicFetchError::Network(error).into();
            assert!(proxy_retryable(&error));
        }
        for error in [
            PublicFetchError::HttpStatus(403),
            PublicFetchError::InvalidResponse,
            PublicFetchError::Network(NetError::ForbiddenDestination),
        ] {
            let error: Error = error.into();
            assert!(!proxy_retryable(&error));
        }
    }

    #[test]
    fn completed_marker_skips_repair_only_while_base_and_overlay_files_exist() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("cobble-forge-repair-{nonce}"));
        fs::create_dir_all(directory.join("versions/1.21.1")).unwrap();
        fs::create_dir_all(directory.join("libraries/example/sample/1")).unwrap();
        let profile: ForgeProfile = serde_json::from_value(serde_json::json!({
            "id":"1.21.1-forge-52.1.0","inheritsFrom":"1.21.1","mainClass":"net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments":{"game":[],"jvm":[]},
            "libraries":[{"name":"example:sample:1","downloads":{"artifact":{"path":"example/sample/1/sample-1.jar","sha1":"0".repeat(40),"size":1,"url":""}}}]
        })).unwrap();
        fs::write(
            directory.join(".cobble-install.json"),
            br#"{"version":"1.21.1","complete":true,"client_only":false}"#,
        )
        .unwrap();
        fs::write(directory.join("versions/1.21.1/1.21.1.json"), b"{}").unwrap();
        fs::write(
            directory.join("versions/1.21.1/1.21.1.jar"),
            b"synthetic client",
        )
        .unwrap();
        let library = directory.join("libraries/example/sample/1/sample-1.jar");
        assert!(!completed_install_ready(&directory, "1.21.1", &profile));
        fs::write(&library, b"synthetic library").unwrap();
        assert!(completed_install_ready(&directory, "1.21.1", &profile));
        fs::remove_file(&library).unwrap();
        assert!(!completed_install_ready(&directory, "1.21.1", &profile));
        fs::write(&library, b"synthetic library").unwrap();
        fs::remove_file(directory.join("versions/1.21.1/1.21.1.jar")).unwrap();
        assert!(!completed_install_ready(&directory, "1.21.1", &profile));
        fs::write(
            directory.join("versions/1.21.1/1.21.1.jar"),
            b"synthetic client",
        )
        .unwrap();
        fs::write(
            directory.join(".cobble-install.json"),
            br#"{"version":"1.21.1","complete":false,"client_only":false}"#,
        )
        .unwrap();
        assert!(!completed_install_ready(&directory, "1.21.1", &profile));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn maven_identity_keeps_classifier_and_extension_when_replacing_versions() {
        assert_eq!(
            maven_identity("example.group:sample:1.0").unwrap(),
            maven_identity("example.group:sample:2.0").unwrap()
        );
        assert_ne!(
            maven_identity("example.group:sample:1.0:client").unwrap(),
            maven_identity("example.group:sample:1.0:universal").unwrap()
        );
        assert_ne!(
            maven_identity("example.group:sample:1.0@zip").unwrap(),
            maven_identity("example.group:sample:1.0@jar").unwrap()
        );
        assert_eq!(
            maven_path("example.group:sample:1.0:client@zip").unwrap(),
            "example/group/sample/1.0/sample-1.0-client.zip"
        );
        for coordinate in [
            "../evil:sample:1",
            "example:../../escape:1",
            "example:sample:1:../../escape",
            "example:sample:1@../exe",
        ] {
            assert!(maven_path(coordinate).is_err());
        }
    }

    #[test]
    fn unsupported_versions_and_profile_family_mismatch_are_explicit() {
        assert!(supported("1.20.1", ForgeKind::Forge, Some("47.1.0")).is_err());
        assert!(supported("1.21.1", ForgeKind::NeoForge, Some("21.2.1")).is_err());
        assert!(supported("1.21.1", ForgeKind::Forge, Some("52.1.0 --flag")).is_err());
        let mut profile: ForgeProfile = serde_json::from_value(serde_json::json!({
            "id":"1.21.1-forge-52.1.0", "inheritsFrom":"1.21.1",
            "mainClass":"net.minecraftforge.bootstrap.ForgeBootstrap",
            "arguments":{"game":["--launchTarget","forge_client"],"jvm":[]},
            "libraries":[{"name":"example.group:sample:1.0:client", "downloads":{"artifact":{
                "path":"example/group/sample/1.0/sample-1.0-client.jar", "sha1":"0".repeat(40), "size":1, "url":""
            }}}]
        })).unwrap();
        assert!(validate_profile(&profile, "1.21.1", "52.1.0", ForgeKind::Forge).is_ok());
        assert!(validate_profile(&profile, "1.21.1", "21.1.264", ForgeKind::NeoForge).is_err());
        for _ in 0..2 {
            profile.libraries.push(serde_json::from_value(serde_json::json!({
                "name":"example.group:sample:2.0:client", "downloads":{"artifact":{
                    "path":"example/group/sample/2.0/sample-2.0-client.jar", "sha1":"0".repeat(40), "size":1, "url":""
                }}
            })).unwrap());
        }
        assert!(validate_profile(&profile, "1.21.1", "52.1.0", ForgeKind::Forge).is_ok());
        normalize_libraries(&mut profile).unwrap();
        assert_eq!(profile.libraries.len(), 1);
        assert_eq!(profile.libraries[0].name, "example.group:sample:2.0:client");
    }
}

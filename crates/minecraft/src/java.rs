// SPDX-License-Identifier: MIT
//! Probe explicitly selected/local Java executables. No runtime installation.
use std::{
    collections::{HashMap, HashSet},
    fmt, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime},
};
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct JavaRuntime {
    pub executable: PathBuf,
    pub version: String,
    pub major: u32,
    pub arch: String,
    pub vendor: String,
}

#[derive(Debug)]
pub enum JavaError {
    NotFound { required_major: u32 },
    InvalidRequirement,
    InvalidExecutable,
    Io(io::Error),
    Timeout,
    Unsuccessful,
    InvalidOutput,
}
impl fmt::Display for JavaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { required_major } => write!(
                f,
                "Java {required_major} matching this computer's architecture was not found"
            ),
            Self::InvalidRequirement => f.write_str("Java major version must be positive"),
            Self::InvalidExecutable => {
                f.write_str("select a Java executable, not a command script")
            }
            Self::Io(error) => write!(f, "could not execute Java: {error}"),
            Self::Timeout => f.write_str("Java probe exceeded five seconds"),
            Self::Unsuccessful => f.write_str("Java probe exited unsuccessfully"),
            Self::InvalidOutput => f.write_str(
                "Java probe did not report valid version, vendor and architecture properties",
            ),
        }
    }
}
impl std::error::Error for JavaError {}

/// Select the newest installed patch with the exact major and host architecture.
pub async fn find_java(required_major: u32) -> Result<JavaRuntime, JavaError> {
    if required_major == 0 {
        return Err(JavaError::InvalidRequirement);
    }
    let runtimes = discover_java(false).await;
    recommend_java(&runtimes, required_major, std::env::consts::ARCH)
        .cloned()
        .ok_or(JavaError::NotFound { required_major })
}

#[derive(Clone)]
struct CachedProbe {
    size: u64,
    modified: SystemTime,
    runtime: Option<JavaRuntime>,
}
static PROBES: OnceLock<Mutex<HashMap<PathBuf, CachedProbe>>> = OnceLock::new();

/// Search bounded, known installation locations. Cache successful and failed
/// probes only while executable metadata stays unchanged. No recursive disk scan.
pub async fn discover_java(rescan: bool) -> Vec<JavaRuntime> {
    let cache = PROBES.get_or_init(Default::default);
    if rescan {
        cache.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
    let mut runtimes = Vec::new();
    let mut seen = HashSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    #[allow(unused_mut)]
    let mut candidates = candidate_paths();
    #[cfg(windows)]
    candidates.extend(registry_candidates().await);
    for path in candidates.into_iter().take(1024) {
        let Ok(path) = std::fs::canonicalize(path) else {
            continue;
        };
        if !seen.insert(path.clone()) {
            continue;
        }
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        let cached = cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&path)
            .filter(|entry| entry.size == metadata.len() && entry.modified == modified)
            .cloned();
        let runtime = if let Some(entry) = cached {
            entry.runtime
        } else {
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            let runtime = tokio::time::timeout_at(deadline, probe_java(&path))
                .await
                .ok()
                .and_then(Result::ok);
            cache.lock().unwrap_or_else(|e| e.into_inner()).insert(
                path.clone(),
                CachedProbe {
                    size: metadata.len(),
                    modified,
                    runtime: runtime.clone(),
                },
            );
            runtime
        };
        if let Some(runtime) = runtime {
            runtimes.push(runtime);
        }
    }
    runtimes.sort_by(|a, b| {
        b.major
            .cmp(&a.major)
            .then_with(|| version_numbers(&b.version).cmp(&version_numbers(&a.version)))
            .then_with(|| a.executable.cmp(&b.executable))
    });
    runtimes
}

#[cfg(windows)]
async fn registry_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for key in [
        r"HKLM\SOFTWARE\JavaSoft",
        r"HKLM\SOFTWARE\WOW6432Node\JavaSoft",
        r"HKCU\SOFTWARE\JavaSoft",
    ] {
        let mut command = Command::new("reg.exe");
        command
            .args(["query", key, "/s", "/v", "JavaHome"])
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .creation_flags(0x0800_0000);
        if let Ok(Ok(output)) = tokio::time::timeout(Duration::from_secs(1), command.output()).await
        {
            for line in String::from_utf8_lossy(&output.stdout).lines().take(256) {
                if let Some((_, home)) = line.split_once("REG_SZ") {
                    candidates.push(PathBuf::from(home.trim()).join("bin/java.exe"));
                }
            }
        }
    }
    candidates
}

pub fn recommend_java<'a>(
    runtimes: &'a [JavaRuntime],
    required_major: u32,
    arch: &str,
) -> Option<&'a JavaRuntime> {
    runtimes
        .iter()
        .filter(|runtime| {
            runtime.major == required_major
                && normalized_arch(&runtime.arch) == normalized_arch(arch)
        })
        .max_by_key(|runtime| version_numbers(&runtime.version))
}

fn normalized_arch(arch: &str) -> &str {
    match arch {
        "amd64" | "x64" | "x86_64" => "x86_64",
        "arm64" | "aarch64" => "aarch64",
        "i386" | "i686" | "x86" => "x86",
        other => other,
    }
}
fn version_numbers(version: &str) -> Vec<u32> {
    version
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

fn add_install_root(candidates: &mut Vec<PathBuf>, root: &Path) {
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    candidates.push(root.join("bin").join(executable));
    candidates.push(root.join("Contents/Home/bin").join(executable));
    let Ok(children) = std::fs::read_dir(root) else {
        return;
    };
    let mut paths: Vec<_> = children
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .take(128)
        .collect();
    paths.sort();
    for path in paths {
        candidates.push(path.join("bin").join(executable));
        candidates.push(path.join("Contents/Home/bin").join(executable));
    }
}

fn candidate_paths() -> Vec<PathBuf> {
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates = Vec::new();
    if let Ok(root) = crate::managed_java_root() {
        add_install_root(&mut candidates, &root);
    }
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(executable));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|path| path.join(executable)));
    }
    if let Some(home) = dirs::home_dir() {
        for relative in [
            ".jdks",
            ".sdkman/candidates/java",
            ".asdf/installs/java",
            ".jenv/versions",
            ".local/share/mise/installs/java",
            "scoop/apps/openjdk/current",
            "scoop/apps/temurin-lts-jdk/current",
            ".jabba/jdk",
        ] {
            add_install_root(&mut candidates, &home.join(relative));
        }
    }
    for root in [
        "/usr/lib/jvm",
        "/usr/java",
        "/opt/java",
        "/Library/Java/JavaVirtualMachines",
        "/opt/homebrew/opt/openjdk",
        "/usr/local/opt/openjdk",
    ] {
        if !cfg!(windows) {
            add_install_root(&mut candidates, Path::new(root));
        }
    }
    #[cfg(windows)]
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            for vendor in [
                "Java",
                "Eclipse Adoptium",
                "Microsoft",
                "Zulu",
                "BellSoft",
                "Amazon Corretto",
                "Programs/Eclipse Adoptium",
            ] {
                add_install_root(&mut candidates, &PathBuf::from(&root).join(vendor));
            }
        }
    }
    candidates
}

/// Direct argv execution, with no shell or inherited JVM option injection.
pub async fn probe_java(path: &Path) -> Result<JavaRuntime, JavaError> {
    #[cfg(windows)]
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(JavaError::InvalidExecutable);
    }
    let executable = std::fs::canonicalize(path).map_err(JavaError::Io)?;
    let mut command = Command::new(&executable);
    command
        .args(["-XshowSettings:properties", "-version"])
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW for desktop callers.
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .map_err(|_| JavaError::Timeout)?
        .map_err(JavaError::Io)?;
    if !output.status.success() {
        return Err(JavaError::Unsuccessful);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_properties(executable, stderr.lines().chain(stdout.lines()))
}

fn parse_properties<'a>(
    executable: PathBuf,
    lines: impl Iterator<Item = &'a str>,
) -> Result<JavaRuntime, JavaError> {
    let (mut version, mut vendor, mut arch) = (None, None, None);
    for line in lines {
        let Some((name, value)) = line.trim().split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match name.trim() {
            "java.version" => version = Some(value.to_owned()),
            "java.vendor" => vendor = Some(value.to_owned()),
            "os.arch" => arch = Some(value.to_owned()),
            _ => {}
        }
    }
    let version = version.ok_or(JavaError::InvalidOutput)?;
    let major = version
        .strip_prefix("1.")
        .unwrap_or(&version)
        .split(['.', '-', '_', '+'])
        .next()
        .and_then(|major| major.parse::<u32>().ok())
        .filter(|major| *major > 0)
        .ok_or(JavaError::InvalidOutput)?;
    Ok(JavaRuntime {
        executable,
        version,
        major,
        arch: arch.ok_or(JavaError::InvalidOutput)?,
        vendor: vendor.ok_or(JavaError::InvalidOutput)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommends_exact_major_architecture_and_newest_numeric_patch() {
        let mut runtimes = Vec::new();
        for (version, arch) in [
            ("21.0.9", "amd64"),
            ("21.0.10", "x86_64"),
            ("21.0.99", "aarch64"),
            ("25.0.1", "amd64"),
        ] {
            let properties =
                format!("java.version = {version}\njava.vendor = fixture\nos.arch = {arch}");
            runtimes.push(parse_properties(PathBuf::from(version), properties.lines()).unwrap());
        }
        assert_eq!(
            recommend_java(&runtimes, 21, "x86_64").unwrap().version,
            "21.0.10"
        );
        assert_eq!(
            recommend_java(&runtimes, 21, "arm64").unwrap().version,
            "21.0.99"
        );
        assert!(recommend_java(&runtimes, 17, "x86_64").is_none());
        assert!(recommend_java(&runtimes, 21, "x86").is_none());
    }

    #[test]
    fn installation_root_search_is_shallow_and_handles_macos_bundles() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(directory.path().join("jdk-21/Contents/Home/bin")).unwrap();
        std::fs::create_dir_all(directory.path().join("unrelated/deep/nested/jdk/bin")).unwrap();
        let mut candidates = Vec::new();
        add_install_root(&mut candidates, directory.path());
        let executable = if cfg!(windows) { "java.exe" } else { "java" };
        assert!(
            candidates.contains(
                &directory
                    .path()
                    .join("jdk-21/Contents/Home/bin")
                    .join(executable)
            )
        );
        assert!(
            !candidates.contains(
                &directory
                    .path()
                    .join("unrelated/deep/nested/jdk/bin")
                    .join(executable)
            )
        );
    }

    #[test]
    fn parses_modern_and_java8_properties_including_spaces() {
        for (version, major) in [("21.0.7", 21), ("1.8.0_432", 8), ("26-ea+9", 26)] {
            let properties = format!(
                "Property settings:\n    java.version = {version}\n    java.vendor = Microsoft Build of OpenJDK\n    os.arch = amd64\nopenjdk version ignored"
            );
            let runtime =
                parse_properties(PathBuf::from("fixture-java"), properties.lines()).unwrap();
            assert_eq!(runtime.major, major);
            assert_eq!(runtime.version, version);
            assert_eq!(runtime.arch, "amd64");
            assert_eq!(runtime.vendor, "Microsoft Build of OpenJDK");
        }
    }

    #[test]
    fn rejects_incomplete_or_invalid_property_output() {
        for output in [
            "java.version = 21",
            "java.version = broken\njava.vendor = fixture\nos.arch = amd64",
            "java.version = 0\njava.vendor = fixture\nos.arch = amd64",
        ] {
            assert!(parse_properties(PathBuf::from("fixture-java"), output.lines()).is_err());
        }
    }
}

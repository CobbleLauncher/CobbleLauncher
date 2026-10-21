// SPDX-License-Identifier: MIT
//! Probe explicitly selected/local Java executables. No runtime installation.
use std::{
    collections::HashSet,
    fmt, io,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
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
                "Java {required_major} was not found in JAVA_HOME or PATH"
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

/// JAVA_HOME takes priority over PATH. Probe each distinct executable and use
/// an exact Java major match; a newer major is not assumed compatible.
pub async fn find_java(required_major: u32) -> Result<JavaRuntime, JavaError> {
    if required_major == 0 {
        return Err(JavaError::InvalidRequirement);
    }
    let executable = if cfg!(windows) { "java.exe" } else { "java" };
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        candidates.push(PathBuf::from(home).join("bin").join(executable));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|path| path.join(executable)));
    }
    let mut seen = HashSet::new();
    for path in candidates {
        let Ok(path) = std::fs::canonicalize(path) else {
            continue;
        };
        if !seen.insert(path.clone()) || !path.is_file() {
            continue;
        }
        if let Ok(runtime) = probe_java(&path).await
            && runtime.major == required_major
        {
            return Ok(runtime);
        }
    }
    Err(JavaError::NotFound { required_major })
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

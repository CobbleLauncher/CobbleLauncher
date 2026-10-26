// SPDX-License-Identifier: MIT
use super::Error;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Archive {
    Zip,
    TarGz,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub os: &'static str,
    pub arch: &'static str,
    pub archive: Archive,
    pub executable: &'static str,
    pub home: &'static str,
}
impl Target {
    pub fn for_platform(os: &str, arch: &str) -> Result<Self, Error> {
        let (os, arch, archive, executable, home) = match (os, arch) {
            ("windows", "x86_64") => ("windows", "x64", Archive::Zip, "bin/java.exe", ""),
            ("linux", "x86_64") => ("linux", "x64", Archive::TarGz, "bin/java", ""),
            ("macos", "x86_64") => (
                "mac",
                "x64",
                Archive::TarGz,
                "Contents/Home/bin/java",
                "Contents/Home",
            ),
            ("macos", "aarch64") => (
                "mac",
                "aarch64",
                Archive::TarGz,
                "Contents/Home/bin/java",
                "Contents/Home",
            ),
            _ => return Err("JAVA_MANAGED_ACQUISITION_UNSUPPORTED_PLATFORM".into()),
        };
        Ok(Self {
            os,
            arch,
            archive,
            executable,
            home,
        })
    }
    pub fn host() -> Result<Self, Error> {
        Self::for_platform(std::env::consts::OS, std::env::consts::ARCH)
    }
    pub fn suffix(self) -> &'static str {
        match self.archive {
            Archive::Zip => ".zip",
            Archive::TarGz => ".tar.gz",
        }
    }
    pub fn key(self) -> String {
        format!("{}-{}", self.os, self.arch)
    }
    pub fn matches_arch(self, arch: &str) -> bool {
        match self.arch {
            "x64" => matches!(arch, "amd64" | "x86_64" | "x64"),
            "aarch64" => matches!(arch, "aarch64" | "arm64"),
            _ => false,
        }
    }
}

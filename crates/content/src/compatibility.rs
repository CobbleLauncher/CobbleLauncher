// SPDX-License-Identifier: MIT
//! One bounded compatibility policy for discovery, import and installation.
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Supported,
    Experimental,
    Unsupported,
}
#[derive(Clone, Debug, Serialize)]
pub struct Compatibility {
    pub status: CompatibilityStatus,
    pub reason_code: &'static str,
    pub java_major: Option<u32>,
    pub loader_prefix: Option<&'static str>,
}
impl Compatibility {
    pub fn allowed(&self) -> bool {
        self.status != CompatibilityStatus::Unsupported
    }
}
/// `None` loader version queries the available adapter family before metadata download.
pub fn query_compatibility(
    mc: &str,
    loader: Option<&str>,
    version: Option<&str>,
    os: &str,
    arch: &str,
) -> Compatibility {
    use CompatibilityStatus::*;
    let unsupported = |reason| Compatibility {
        status: Unsupported,
        reason_code: reason,
        java_major: None,
        loader_prefix: None,
    };
    if !matches!(os, "windows" | "linux" | "macos") || !matches!(arch, "x86_64" | "aarch64") {
        return unsupported("platform_unsupported");
    }
    let loader = loader.unwrap_or("vanilla");
    let (major, prefix, established) = match (mc, loader) {
        ("1.21.1", "forge") => (21, Some("52."), true),
        ("1.20.1", "forge") => (17, Some("47.4."), false),
        ("1.21.1", "neoforge") => (21, Some("21.1."), true),
        ("1.21.4", "neoforge") => (21, Some("21.4."), false),
        (_, "forge" | "neoforge") => return unsupported("loader_target_unsupported"),
        (_, "vanilla" | "minecraft" | "fabric") => {
            if mc.is_empty()
                || mc.len() > 128
                || !mc
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
            {
                return unsupported("minecraft_unsupported");
            }
            // Mojang metadata remains authoritative for these adapters' Java requirement.
            return Compatibility {
                status: Experimental,
                reason_code: "metadata_required",
                java_major: None,
                loader_prefix: None,
            };
        }
        _ => return unsupported("loader_unsupported"),
    };
    if let Some(version) = version
        && (version.len() > 64
            || version.split('.').count() != 3
            || !version.starts_with(prefix.unwrap())
            || !version
                .split('.')
                .all(|part| !part.is_empty() && part.parse::<u32>().is_ok()))
    {
        return unsupported("loader_version_unsupported");
    }
    let verified_platform = os == "windows" && arch == "x86_64";
    Compatibility {
        status: if established && verified_platform {
            Supported
        } else {
            Experimental
        },
        reason_code: if established && verified_platform {
            "verified_target"
        } else {
            "launch_unverified"
        },
        java_major: Some(major),
        loader_prefix: prefix,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_snapshot_ids_defer_to_metadata() {
        for mc in ["25w42a", "1.21.4-rc1"] {
            assert!(query_compatibility(mc, Some("fabric"), None, "windows", "x86_64").allowed());
        }
    }
    #[test]
    fn bounded_loader_lines_and_java() {
        let q = |mc, kind, v| query_compatibility(mc, Some(kind), Some(v), "windows", "x86_64");
        assert_eq!(q("1.20.1", "forge", "47.4.28").java_major, Some(17));
        assert_eq!(
            q("1.21.4", "neoforge", "21.4.157").status,
            CompatibilityStatus::Experimental
        );
        for (mc, k, v) in [
            ("1.20.1", "forge", "47.3.0"),
            ("1.21.4", "neoforge", "21.1.9"),
            ("1.21.1", "forge", "52.1.0 --flag"),
            ("1.21.4", "neoforge", "21.4."),
            ("1.21.1", "forge", "52.1"),
            ("1.21.1", "forge", "52.1.0.1"),
        ] {
            assert!(!q(mc, k, v).allowed());
        }
        assert_eq!(
            query_compatibility("1.21.1", Some("forge"), Some("52.1.0"), "linux", "aarch64").status,
            CompatibilityStatus::Experimental
        );
    }
}

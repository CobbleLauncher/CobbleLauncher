// SPDX-License-Identifier: MIT
//! Explicit local verification. Inspection never fetches metadata or launches a game.
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{Dir, OpenOptions};
use cobble_content::{EnvironmentSelection, EnvironmentSource};
use cobble_minecraft::{FileKind, PlannedFile, PlatformContext, VersionMetadata};
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};
type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectedContent {
    pub selection: EnvironmentSelection,
    pub filename: String,
    pub sha1: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadinessIssue {
    pub category: String,
    pub path: Option<String>,
    pub state: String,
    pub size_bytes: Option<u64>,
    pub repairable: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LocalReadiness {
    pub ready: bool,
    pub checked_files: usize,
    pub ready_files: usize,
    pub missing_files: usize,
    pub changed_files: usize,
    pub unknown: usize,
    pub issues: Vec<ReadinessIssue>,
    pub issues_truncated: bool,
    pub required_java: Option<u32>,
    pub content_revision: String,
    pub selected_content: Vec<SelectedContent>,
}
impl LocalReadiness {
    fn issue(
        &mut self,
        category: &str,
        path: Option<String>,
        state: &str,
        size: Option<u64>,
        repairable: bool,
    ) {
        match state {
            "missing" => self.missing_files += 1,
            "changed" => self.changed_files += 1,
            _ => self.unknown += 1,
        }
        if self.issues.len() < 512 {
            self.issues.push(ReadinessIssue {
                category: category.into(),
                path,
                state: state.into(),
                size_bytes: size,
                repairable,
            });
        } else {
            self.issues_truncated = true;
        }
    }
}

pub(crate) fn open_scoped(root: &Dir, relative: &str) -> Result<Option<std::fs::File>, Error> {
    let safe = SafeRelativePath::parse(relative)?;
    let mut parts = safe.as_str().split('/').peekable();
    let mut current = root.try_clone()?;
    while let Some(part) = parts.next() {
        if parts.peek().is_some() {
            current = match current.open_dir_nofollow(part) {
                Ok(dir) => dir,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.into()),
            };
        } else {
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            return match current.open_with(part, &options) {
                Ok(file) if file.metadata()?.is_file() => Ok(Some(file.into_std())),
                Ok(_) => Err("READINESS_NON_REGULAR_FILE".into()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            };
        }
    }
    Err("READINESS_PATH_INVALID".into())
}

fn bytes(root: &Dir, path: &str, limit: u64) -> Result<Option<Vec<u8>>, Error> {
    let Some(file) = open_scoped(root, path)? else {
        return Ok(None);
    };
    if file.metadata()?.len() > limit {
        return Err("READINESS_METADATA_LIMIT".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("READINESS_METADATA_LIMIT".into());
    }
    Ok(Some(bytes))
}

pub(crate) fn file_matches(
    root: &Dir,
    path: &str,
    size: Option<u64>,
    sha1: Option<&str>,
    control: &crate::OperationControl,
) -> Result<Option<bool>, Error> {
    control.check_cancelled()?;
    let Some(mut file) = open_scoped(root, path)? else {
        return Ok(None);
    };
    if size.is_some_and(|s| file.metadata().is_ok_and(|m| m.len() != s)) {
        return Ok(Some(false));
    }
    if let Some(expected) = sha1 {
        if expected.len() != 40 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("READINESS_HASH_INVALID".into());
        }
        let mut digest = Sha1::new();
        let mut chunk = [0u8; 65536];
        loop {
            control.check_cancelled()?;
            let n = file.read(&mut chunk)?;
            if n == 0 {
                break;
            }
            digest.update(&chunk[..n]);
        }
        return Ok(Some(
            format!("{:x}", digest.finalize()).eq_ignore_ascii_case(expected),
        ));
    }
    Ok(Some(true))
}

fn check(
    report: &mut LocalReadiness,
    root: &Dir,
    path: &str,
    size: Option<u64>,
    hash: Option<&str>,
    category: &str,
    control: &crate::OperationControl,
) -> Result<(), Error> {
    report.checked_files += 1;
    match file_matches(root, path, size, hash, control)? {
        Some(true) => report.ready_files += 1,
        Some(false) => report.issue(category, Some(path.into()), "changed", size, false),
        None => report.issue(category, Some(path.into()), "missing", size, true),
    }
    Ok(())
}

pub fn selected_content(directory: &Path) -> Result<Vec<SelectedContent>, Error> {
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let mut values = Vec::new();
    for (source, provider) in [
        (
            EnvironmentSource::Modrinth,
            crate::modrinth::ModProvider::Modrinth,
        ),
        (
            EnvironmentSource::Curseforge,
            crate::modrinth::ModProvider::CurseForge,
        ),
    ] {
        for (project, item) in crate::modrinth::read_provider_provenance(&root, provider)? {
            let path = SafeRelativePath::parse(&item.filename)?;
            if path.as_str().contains('/') || !path.as_str().to_ascii_lowercase().ends_with(".jar")
            {
                return Err("READINESS_CONTENT_INVALID".into());
            }
            values.push(SelectedContent {
                selection: EnvironmentSelection {
                    source,
                    project_id: project,
                    version_id: item.version_id,
                },
                filename: item.filename,
                sha1: item.sha1,
            });
        }
    }
    if values.len() > 256 {
        return Err("READINESS_CONTENT_LIMIT".into());
    }
    Ok(values)
}
pub fn content_revision(selected: &[SelectedContent]) -> Result<String, Error> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(selected)?)
    ))
}

/// The caller owns a stopped-instance lease. Every path comes from validated
/// installed metadata or managed provenance; unrelated user content is not scanned.
pub fn inspect_local_readiness(
    version: &str,
    directory: &Path,
    loader_kind: Option<&str>,
    loader_version: Option<&str>,
    control: crate::OperationControl,
) -> Result<LocalReadiness, Error> {
    control.check_cancelled()?;
    let root = Dir::open_ambient_dir(directory, cap_std::ambient_authority())?;
    let mut report = LocalReadiness::default();
    let install = bytes(&root, ".cobble-install.json", 65536)?;
    let installed = install
        .as_ref()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(b).ok())
        .is_some_and(|v| {
            v["version"] == version && v["complete"] == true && v["client_only"] == false
        });
    if !installed {
        report.issue("metadata", None, "incomplete_install", None, true);
    }
    let selected = selected_content(directory)?;
    report.content_revision = content_revision(&selected)?;
    report.selected_content = selected;
    if crate::environment_install::ensure_environment_complete(directory).is_err() {
        report.issue("content", None, "pending_install", None, false);
    }
    if crate::modrinth::ensure_mod_install_complete(directory).is_err()
        || crate::modupdates::ensure_update_complete(directory).is_err()
    {
        report.issue("content", None, "pending_operation", None, false);
    }
    let path = format!("versions/{version}/{version}.json");
    let raw = bytes(&root, &path, 16 * 1024 * 1024)?;
    if let Some(raw) = raw {
        let metadata = VersionMetadata::parse_installed(&raw, version)?;
        report.required_java = Some(metadata.java_version.major_version);
        let plan = metadata.download_plan(&PlatformContext::current()?)?;
        let mut overridden = BTreeSet::new();
        if let Some(kind) = loader_kind {
            let overlay_valid = match kind {
                "fabric" => crate::fabric::load_overlay(directory, version)
                    .ok()
                    .flatten()
                    .is_some_and(|s| Some(s.loader_version.as_str()) == loader_version),
                "forge" | "neoforge" => crate::forge::load_overlay(directory, version)
                    .ok()
                    .flatten()
                    .is_some_and(|s| {
                        Some(s.loader_version.as_str()) == loader_version && s.kind.id() == kind
                    }),
                _ => false,
            };
            let marker = if kind == "fabric" {
                ".cobble-fabric.json"
            } else {
                ".cobble-forge.json"
            };
            if let Some(state) = bytes(&root, marker, 4 * 1024 * 1024)? {
                let value: serde_json::Value = serde_json::from_slice(&state)?;
                if !overlay_valid
                    || value["complete"] != true
                    || value["mc_version"] != version
                    || value["loader_version"].as_str() != loader_version
                {
                    report.issue("loader", None, "incomplete", None, true);
                } else {
                    let libraries = value["profile"]["libraries"]
                        .as_array()
                        .ok_or("READINESS_LOADER_INVALID")?;
                    if libraries.len() > 4096 {
                        return Err("READINESS_LOADER_LIMIT".into());
                    }
                    for lib in libraries {
                        let name = lib["name"].as_str().ok_or("READINESS_LOADER_INVALID")?;
                        let path = if kind == "fabric" {
                            let library: crate::fabric::FabricLibrary =
                                serde_json::from_value(lib.clone())?;
                            overridden.insert(format!("{}:@jar", library.identity()));
                            library.path()?.as_str().to_owned()
                        } else {
                            let library: crate::forge::ForgeLibrary =
                                serde_json::from_value(lib.clone())?;
                            overridden.insert(library.identity()?);
                            library.path()?.as_str().to_owned()
                        };
                        let artifact = if kind == "fabric" {
                            lib
                        } else {
                            &lib["downloads"]["artifact"]
                        };
                        check(
                            &mut report,
                            &root,
                            &path,
                            artifact["size"].as_u64(),
                            artifact["sha1"].as_str(),
                            "loader",
                            &control,
                        )?;
                        let _ = name;
                    }
                }
            } else {
                report.issue("loader", Some(marker.into()), "missing", None, true);
            }
        }
        let raw_value: serde_json::Value = serde_json::from_slice(&raw)?;
        let replaced: BTreeSet<&str> = raw_value["libraries"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|lib| {
                let identity = crate::forge::maven_identity(lib["name"].as_str()?).ok()?;
                overridden
                    .contains(&identity)
                    .then(|| lib["downloads"]["artifact"]["path"].as_str())
                    .flatten()
            })
            .collect();
        let index_path = format!("assets/indexes/{}.json", metadata.asset_index.id);
        for file in plan.files {
            if file.kind == FileKind::Library
                && replaced.contains(file.path.as_str().trim_start_matches("libraries/"))
            {
                continue;
            }
            check(
                &mut report,
                &root,
                file.path.as_str(),
                Some(file.size),
                Some(&hex(&file.sha1)),
                if matches!(file.kind, FileKind::Asset | FileKind::AssetIndex) {
                    "assets"
                } else {
                    "libraries"
                },
                &control,
            )?;
        }
        if file_matches(
            &root,
            &index_path,
            Some(metadata.asset_index.size),
            Some(&metadata.asset_index.sha1),
            &control,
        )? == Some(true)
        {
            let index = bytes(&root, &index_path, 32 * 1024 * 1024)?
                .ok_or("READINESS_ASSET_INDEX_MISSING")?;
            for file in asset_files(&index)? {
                check(
                    &mut report,
                    &root,
                    file.path.as_str(),
                    Some(file.size),
                    Some(&hex(&file.sha1)),
                    "assets",
                    &control,
                )?;
            }
        } else {
            report.issue("assets", None, "index_unavailable", None, true);
        }
    } else {
        report.issue("metadata", Some(path), "missing", None, true);
    }
    for item in report.selected_content.clone() {
        let enabled = format!("mods/{}", item.filename);
        let disabled = format!("{enabled}.disabled");
        let path = if open_scoped(&root, &enabled)?.is_some() {
            enabled
        } else if open_scoped(&root, &disabled)?.is_some() {
            disabled
        } else {
            enabled
        };
        check(
            &mut report,
            &root,
            &path,
            None,
            Some(&item.sha1),
            "content",
            &control,
        )?;
    }
    report.ready = report.missing_files == 0 && report.changed_files == 0 && report.unknown == 0;
    Ok(report)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
#[path = "readiness/tests.rs"]
mod tests;

#[derive(Deserialize)]
struct AssetIndex {
    objects: BTreeMap<String, AssetObject>,
    #[serde(default)]
    r#virtual: bool,
    #[serde(default)]
    map_to_resources: bool,
}
#[derive(Deserialize)]
struct AssetObject {
    hash: String,
    size: u64,
}
pub(crate) fn asset_files(bytes: &[u8]) -> Result<Vec<PlannedFile>, Error> {
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("READINESS_ASSET_INDEX_LIMIT".into());
    }
    let index: AssetIndex = serde_json::from_slice(bytes)?;
    if index.r#virtual || index.map_to_resources || index.objects.len() > 300_000 {
        return Err("READINESS_ASSET_INDEX_UNSUPPORTED".into());
    }
    let unique: BTreeMap<_, _> = index
        .objects
        .into_values()
        .map(|o| (o.hash.clone(), o))
        .collect();
    unique
        .into_values()
        .map(|o| {
            if o.hash.len() != 40
                || !o.hash.bytes().all(|b| b.is_ascii_hexdigit())
                || o.size == 0
                || o.size > 512 * 1024 * 1024
            {
                return Err("READINESS_ASSET_INVALID".into());
            }
            let mut digest = [0u8; 20];
            for (i, b) in digest.iter_mut().enumerate() {
                *b = u8::from_str_radix(&o.hash[i * 2..i * 2 + 2], 16)?;
            }
            Ok(PlannedFile {
                path: SafeRelativePath::parse(&format!(
                    "assets/objects/{}/{}",
                    &o.hash[..2],
                    o.hash
                ))?,
                url: format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    &o.hash[..2],
                    o.hash
                ),
                size: o.size,
                sha1: digest,
                kind: FileKind::Asset,
            })
        })
        .collect()
}

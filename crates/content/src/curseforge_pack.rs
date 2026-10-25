// SPDX-License-Identifier: MIT
//! Read-only parsing of CurseForge's local modpack ZIP manifest.
//! Project/file IDs are metadata only; resolving or downloading them is a separate step.

use crate::{BoxError, PackOverride, mrpack::Namespace};
use cobble_store::SafeRelativePath;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

const MAX_MANIFEST: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct CurseForgePack {
    pub name: String,
    pub version: String,
    pub minecraft_version: String,
    pub loader: Option<CurseForgePackLoader>,
    pub files: Vec<CurseForgePackFile>,
    pub overrides: Vec<PackOverride>,
}

#[derive(Clone, Debug)]
pub struct CurseForgePackLoader {
    pub kind: String,
    pub version: String,
}

#[derive(Clone, Debug)]
pub struct CurseForgePackFile {
    pub project_id: u32,
    pub file_id: u32,
    pub required: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    manifest_type: String,
    manifest_version: u32,
    name: String,
    version: String,
    minecraft: Minecraft,
    files: Vec<ManifestFile>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Minecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ModLoader>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "required_by_default")]
    required: bool,
}

fn default_overrides() -> String {
    "overrides".to_owned()
}

fn required_by_default() -> bool {
    true
}

/// Parses the local ZIP plan without network access, extraction, or execution.
pub fn read_curseforge_pack(path: &Path) -> Result<CurseForgePack, BoxError> {
    read_curseforge_pack_file(File::open(path)?)
}

/// Takes ownership of the selected archive. Override extraction must reuse the same pinned bytes.
pub fn read_curseforge_pack_file<R: Read + Seek>(source: R) -> Result<CurseForgePack, BoxError> {
    let mut source = source;
    if source.seek(SeekFrom::End(0))? > MAX_TOTAL {
        return Err(invalid("CurseForge pack archive exceeds 16 GiB").into());
    }
    source.seek(SeekFrom::Start(0))?;
    let mut archive = zip::ZipArchive::new(source)?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("CurseForge pack contains too many ZIP entries").into());
    }
    archive.validate_header_consistency()?;

    let mut archive_names = Namespace::default();
    let mut destinations = Namespace::default();
    let mut manifest_bytes = None;
    let mut overrides = Vec::new();
    let mut expanded = 0u64;

    for entry_index in 0..archive.len() {
        let mut entry = archive.by_index(entry_index)?;
        let name = std::str::from_utf8(entry.name_raw())?;
        let directory = entry.is_dir();
        if entry.encrypted() {
            return Err(invalid("Encrypted CurseForge pack entries are unsupported").into());
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != if directory { 0o040000 } else { 0o100000 } {
                return Err(
                    invalid("CurseForge pack links and special files are forbidden").into(),
                );
            }
        }
        let raw_path = if directory {
            name.strip_suffix('/').unwrap_or(name)
        } else {
            name
        };
        let safe = SafeRelativePath::parse(raw_path)?;
        archive_names.insert(safe.as_str(), directory, Some(0))?;
        if entry.size() > MAX_FILE {
            return Err(invalid("CurseForge pack entry exceeds 2 GiB").into());
        }
        expanded = expanded.saturating_add(entry.size());
        if expanded > MAX_TOTAL {
            return Err(invalid("CurseForge pack expanded content exceeds 16 GiB").into());
        }

        if safe.as_str() == "manifest.json" {
            if directory || manifest_bytes.is_some() || entry.size() > MAX_MANIFEST {
                return Err(
                    invalid("CurseForge manifest is duplicate, a directory, or too large").into(),
                );
            }
            let mut bytes = Vec::new();
            entry
                .by_ref()
                .take(MAX_MANIFEST + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_MANIFEST {
                return Err(invalid("CurseForge manifest exceeds 4 MiB").into());
            }
            manifest_bytes = Some(bytes);
        }
    }

    let manifest: Manifest = serde_json::from_slice(
        &manifest_bytes.ok_or_else(|| invalid("Missing root manifest.json"))?,
    )?;
    if manifest.manifest_type != "minecraftModpack" || manifest.manifest_version != 1 {
        return Err(invalid("Unsupported CurseForge manifest type or version").into());
    }
    validate_text(&manifest.name, "pack name")?;
    validate_text(&manifest.version, "pack version")?;
    validate_text(&manifest.minecraft.version, "Minecraft version")?;
    if manifest.files.len() > MAX_ENTRIES {
        return Err(invalid("CurseForge pack declares too many files").into());
    }

    let mut files = Vec::with_capacity(manifest.files.len());
    let mut unique_files = std::collections::HashSet::new();
    for file in manifest.files {
        if file.project_id == 0
            || file.file_id == 0
            || !unique_files.insert((file.project_id, file.file_id))
        {
            return Err(invalid(
                "CurseForge pack contains an invalid or duplicate project/file ID",
            )
            .into());
        }
        files.push(CurseForgePackFile {
            project_id: file.project_id,
            file_id: file.file_id,
            required: file.required,
        });
    }

    let loader = parse_loader(&manifest.minecraft.mod_loaders)?;
    if let Some(selected) = &loader
        && !crate::query_compatibility(
            &manifest.minecraft.version,
            Some(&selected.kind),
            Some(&selected.version),
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
        .allowed()
    {
        return Err(invalid("Unsupported CurseForge Minecraft or loader target").into());
    }
    let override_root = SafeRelativePath::parse(&manifest.overrides)?
        .as_str()
        .to_owned();
    let override_prefix = format!("{override_root}/");
    for entry_index in 0..archive.len() {
        let entry = archive.by_index(entry_index)?;
        if entry.is_dir() {
            continue;
        }
        let raw = std::str::from_utf8(entry.name_raw())?;
        let safe = SafeRelativePath::parse(raw)?;
        if let Some(relative) = safe.as_str().strip_prefix(&override_prefix) {
            let path = validate_destination(relative)?;
            destinations.insert(&path, false, Some(0))?;
            overrides.push(PackOverride {
                entry_index,
                path,
                layer: 0,
                size: entry.size(),
            });
        }
    }
    overrides.sort_by_key(|entry| entry.entry_index);

    Ok(CurseForgePack {
        name: manifest.name,
        version: manifest.version,
        minecraft_version: manifest.minecraft.version,
        loader,
        files,
        overrides,
    })
}

fn parse_loader(loaders: &[ModLoader]) -> Result<Option<CurseForgePackLoader>, BoxError> {
    let mut primary = loaders.iter().filter(|loader| loader.primary);
    let Some(loader) = primary.next() else {
        return Ok(None);
    };
    if primary.next().is_some() {
        return Err(invalid("CurseForge pack declares multiple primary mod loaders").into());
    }
    let (kind, version) = loader
        .id
        .split_once('-')
        .ok_or_else(|| invalid("Invalid CurseForge mod loader ID"))?;
    if !matches!(kind, "fabric" | "quilt" | "forge" | "neoforge") {
        return Err(invalid("Unsupported CurseForge mod loader").into());
    }
    validate_text(version, "mod loader version")?;
    SafeRelativePath::parse(version)?;
    Ok(Some(CurseForgePackLoader {
        kind: kind.to_owned(),
        version: version.to_owned(),
    }))
}

fn validate_text(value: &str, label: &str) -> Result<(), BoxError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(invalid(&format!("Invalid CurseForge {label}")).into());
    }
    Ok(())
}

fn validate_destination(path: &str) -> Result<String, BoxError> {
    let safe = SafeRelativePath::parse(path)?;
    let first = safe.as_str().split('/').next().unwrap_or("").to_lowercase();
    if first.starts_with(".cobble")
        || matches!(
            first.as_str(),
            "versions"
                | "libraries"
                | "assets"
                | "runtime"
                | "natives"
                | "java"
                | "launcher_profiles.json"
                | "launcher_accounts.json"
        )
    {
        return Err(invalid(
            "CurseForge override overlaps protected launcher or game installation files",
        )
        .into());
    }
    Ok(safe.as_str().to_owned())
}

/// Validate the final platform-independent namespace before any publication.
/// Exact override paths replace downloaded files; aliases and file/directory
/// conflicts never acquire platform-dependent precedence.
pub fn validate_curseforge_pack_paths(
    pack: &CurseForgePack,
    download_paths: &[String],
) -> Result<(), BoxError> {
    let mut namespace = Namespace::default();
    for path in download_paths {
        namespace.insert(&validate_destination(path)?, false, Some(0))?;
    }
    for entry in &pack.overrides {
        namespace.insert(&entry.path, false, Some(1))?;
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Cursor, Write};
    use zip::write::SimpleFileOptions;

    fn archive(manifest: serde_json::Value, entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.start_file("manifest.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(&manifest).unwrap())
            .unwrap();
        for (path, bytes) in entries {
            writer.start_file(*path, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn manifest() -> serde_json::Value {
        json!({
            "manifestType":"minecraftModpack", "manifestVersion":1,
            "name":"Fixture Pack", "version":"1.0.0", "overrides":"overrides",
            "minecraft":{"version":"1.20.1","modLoaders":[{"id":"fabric-0.15.11","primary":true}]},
            "files":[{"projectID":123,"fileID":456},{"projectID":789,"fileID":987,"required":false}]
        })
    }

    #[test]
    fn parses_manifest_files_loader_and_safe_override_plan() {
        let bytes = archive(manifest(), &[("overrides/config/mod.json", b"{}")]);
        let plan = read_curseforge_pack_file(Cursor::new(bytes)).unwrap();
        assert_eq!(plan.name, "Fixture Pack");
        assert_eq!(plan.minecraft_version, "1.20.1");
        assert_eq!(plan.loader.unwrap().kind, "fabric");
        assert_eq!(plan.files.len(), 2);
        assert!(plan.files[0].required);
        assert!(!plan.files[1].required);
        assert_eq!(plan.overrides.len(), 1);
        assert_eq!(plan.overrides[0].path, "config/mod.json");
        assert_eq!(plan.overrides[0].size, 2);
    }

    #[test]
    fn rejects_bad_identity_unsupported_loader_and_unsafe_override() {
        let mut invalid_id = manifest();
        invalid_id["files"][0]["fileID"] = 0.into();
        assert!(read_curseforge_pack_file(Cursor::new(archive(invalid_id, &[]))).is_err());

        let mut unsupported_loader = manifest();
        unsupported_loader["minecraft"]["modLoaders"][0]["id"] = "unknown-1".into();
        assert!(read_curseforge_pack_file(Cursor::new(archive(unsupported_loader, &[]))).is_err());

        assert!(
            read_curseforge_pack_file(Cursor::new(archive(
                manifest(),
                &[("overrides/../escape.txt", b"bad")],
            )))
            .is_err()
        );
        assert!(
            read_curseforge_pack_file(Cursor::new(archive(
                manifest(),
                &[("overrides/versions/1.20.1/evil.jar", b"bad")],
            )))
            .is_err()
        );
    }
}

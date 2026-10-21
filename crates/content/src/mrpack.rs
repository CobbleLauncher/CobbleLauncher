// SPDX-License-Identifier: MIT
//! Read-only Modrinth format v1 parsing, from the public Modrinth specification.
//! Overrides refer to ZIP entry indices: installation must use the same pinned
//! archive bytes, bound decompression and verify CRC before publishing files.

use crate::BoxError;
use cobble_store::SafeRelativePath;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;
use url::Url;

const MAX_INDEX: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 50_000;
const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_TOTAL: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Mrpack {
    pub name: String,
    pub version_id: String,
    pub summary: Option<String>,
    pub minecraft_version: String,
    pub loader_kind: Option<String>,
    pub loader_version: Option<String>,
    pub files: Vec<MrpackFile>,
    pub overrides: Vec<PackOverride>,
}

#[derive(Clone, Debug)]
pub struct MrpackFile {
    pub path: String,
    pub downloads: Vec<String>,
    pub file_size: u64,
    pub sha1: String,
    pub sha512: String,
    pub optional: bool,
}

#[derive(Clone, Debug)]
pub struct PackOverride {
    pub entry_index: usize,
    pub path: String,
    pub layer: u8,
    pub size: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    format_version: u32,
    game: String,
    version_id: String,
    name: String,
    summary: Option<String>,
    files: Vec<IndexFile>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    path: String,
    hashes: BTreeMap<String, String>,
    env: Option<Environment>,
    downloads: Vec<String>,
    file_size: u64,
}

#[derive(Deserialize)]
struct Environment {
    client: Side,
    server: Side,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Required,
    Optional,
    Unsupported,
}

/// No extraction, network access or execution occurs here.
pub fn read_mrpack(path: &Path) -> Result<Mrpack, BoxError> {
    read_mrpack_file(File::open(path)?)
}

/// Takes ownership of an already pinned archive handle. Extraction must reuse
/// this file's bytes (for example via a caller-owned immutable snapshot).
pub fn read_mrpack_file(source: File) -> Result<Mrpack, BoxError> {
    if source.metadata()?.len() > MAX_TOTAL {
        return Err(invalid("MRPACK archive exceeds 16 GiB").into());
    }
    let mut archive = zip::ZipArchive::new(source)?;
    if archive.len() > MAX_ENTRIES {
        return Err(invalid("MRPACK contains too many entries").into());
    }
    // zip's consistency check also rejects duplicate central names (which its
    // name lookup otherwise collapses), overlapping entries and header aliases.
    archive.validate_header_consistency()?;
    let mut archive_names = Namespace::default();
    let mut destinations = Namespace::default();
    let mut index_bytes = None;
    let mut overrides = Vec::new();
    let mut total = 0;
    for entry_index in 0..archive.len() {
        let mut entry = archive.by_index(entry_index)?;
        let name = std::str::from_utf8(entry.name_raw())?.to_owned();
        if entry.encrypted() {
            return Err(invalid("Encrypted MRPACK entries are unsupported").into());
        }
        let directory = entry.is_dir();
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != if directory { 0o040000 } else { 0o100000 } {
                return Err(invalid("MRPACK links and special files are forbidden").into());
            }
        }
        let raw_path = if directory {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        };
        let safe = SafeRelativePath::parse(raw_path)?;
        archive_names.insert(safe.as_str(), directory, Some(0))?;
        if entry.size() > MAX_FILE {
            return Err(invalid("MRPACK entry exceeds 2 GiB").into());
        }
        total += entry.size();
        if total > MAX_TOTAL {
            return Err(invalid("MRPACK expanded content exceeds 16 GiB").into());
        }
        if safe.as_str() == "modrinth.index.json" {
            if directory || index_bytes.is_some() || entry.size() > MAX_INDEX {
                return Err(invalid("MRPACK index is duplicate, a directory, or too large").into());
            }
            let mut bytes = Vec::new();
            entry.by_ref().take(MAX_INDEX + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_INDEX {
                return Err(invalid("MRPACK index exceeds 4 MiB").into());
            }
            index_bytes = Some(bytes);
        } else if let Some((relative, layer)) = safe
            .as_str()
            .strip_prefix("overrides/")
            .map(|p| (p, 0))
            .or_else(|| {
                safe.as_str()
                    .strip_prefix("client-overrides/")
                    .map(|p| (p, 1))
            })
        {
            let destination = destination(relative)?;
            destinations.insert(&destination, directory, Some(layer + 1))?;
            if !directory {
                overrides.push(PackOverride {
                    entry_index,
                    path: destination,
                    layer,
                    size: entry.size(),
                });
            }
        }
    }
    let bytes = index_bytes.ok_or_else(|| invalid("Missing root modrinth.index.json"))?;
    let index: Index = serde_json::from_slice(&bytes)?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(invalid("Unsupported MRPACK format version or game").into());
    }
    if index.files.len() > MAX_ENTRIES
        || index.name.trim().is_empty()
        || index.version_id.trim().is_empty()
    {
        return Err(invalid("Invalid MRPACK name, version ID or file count").into());
    }
    for (dependency, version) in &index.dependencies {
        if !matches!(
            dependency.as_str(),
            "minecraft" | "fabric-loader" | "forge" | "neoforge"
        ) {
            return Err(invalid(&format!("Unsupported MRPACK dependency: {dependency}")).into());
        }
        if version.is_empty()
            || version.len() > 128
            || version.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(invalid("Invalid MRPACK dependency version").into());
        }
        SafeRelativePath::parse(version)?;
    }
    let minecraft_version = index
        .dependencies
        .get("minecraft")
        .ok_or_else(|| invalid("MRPACK requires a minecraft dependency"))?
        .clone();
    let mut loader_kind = None;
    let mut loader_version = None;
    for (dependency, kind) in [
        ("fabric-loader", "fabric"),
        ("forge", "forge"),
        ("neoforge", "neoforge"),
    ] {
        if let Some(version) = index.dependencies.get(dependency) {
            if loader_kind.is_some() {
                return Err(invalid(
                    "Conflicting MRPACK loader dependencies: declare only one loader",
                )
                .into());
            }
            loader_kind = Some(kind.to_owned());
            loader_version = Some(version.clone());
        }
    }
    if matches!(loader_kind.as_deref(), Some("forge" | "neoforge")) && minecraft_version != "1.21.1"
    {
        return Err(invalid(
            "MRPACK Forge and NeoForge imports currently support Minecraft 1.21.1 only",
        )
        .into());
    }
    let mut files = Vec::new();
    for file in index.files {
        let path = destination(&file.path)?;
        // Validate even server-only entries; bad paths do not become acceptable
        // merely because the current environment would skip their content.
        destinations.insert(&path, false, Some(0))?;
        let sha1 = hash(&file.hashes, "sha1", 40)?;
        let sha512 = hash(&file.hashes, "sha512", 128)?;
        if file.file_size > MAX_FILE || file.downloads.is_empty() || file.downloads.len() > 32 {
            return Err(invalid("MRPACK download size or URL count is invalid").into());
        }
        total += file.file_size;
        if total > MAX_TOTAL {
            return Err(invalid("MRPACK total content exceeds 16 GiB").into());
        }
        for download in &file.downloads {
            validate_download(download)?;
        }
        if let Some(env) = &file.env {
            let _ = &env.server;
            if matches!(env.client, Side::Unsupported) {
                continue;
            }
        }
        files.push(MrpackFile {
            path,
            downloads: file.downloads,
            file_size: file.file_size,
            sha1,
            sha512,
            optional: file
                .env
                .is_some_and(|env| matches!(env.client, Side::Optional)),
        });
    }
    overrides.sort_by_key(|entry| (entry.layer, entry.entry_index));
    Ok(Mrpack {
        name: index.name,
        version_id: index.version_id,
        summary: index.summary,
        minecraft_version,
        loader_kind,
        loader_version,
        files,
        overrides,
    })
}

fn destination(path: &str) -> Result<String, BoxError> {
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
            "MRPACK destination overlaps protected launcher or game installation files",
        )
        .into());
    }
    Ok(safe.as_str().to_owned())
}

fn hash(hashes: &BTreeMap<String, String>, name: &str, length: usize) -> Result<String, BoxError> {
    let value = hashes
        .get(name)
        .ok_or_else(|| invalid("MRPACK requires both sha1 and sha512"))?;
    if value.len() != length || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Invalid MRPACK digest").into());
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_download(value: &str) -> Result<(), BoxError> {
    if value.len() > 8192
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
        || value.contains('\\')
    {
        return Err(invalid("MRPACK download URL contains forbidden characters").into());
    }
    let url = Url::parse(value)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || !matches!(
            url.host_str(),
            Some("cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com")
        )
    {
        return Err(invalid("Unsupported MRPACK URL: HTTPS, no credentials and an approved download host are required").into());
    }
    Ok(())
}

#[derive(Default)]
struct Namespace {
    nodes: HashMap<String, (String, bool)>,
    explicit: HashSet<(String, u8)>,
}

impl Namespace {
    fn insert(&mut self, path: &str, directory: bool, layer: Option<u8>) -> Result<(), BoxError> {
        let mut parent = String::new();
        let components: Vec<_> = path.split('/').collect();
        for (position, part) in components.iter().enumerate() {
            let last = position + 1 == components.len();
            let is_directory = !last || directory;
            let compatible: String = part.nfkc().collect();
            let alias: String = caseless::default_case_fold_str(&compatible)
                .chars()
                .flat_map(char::to_uppercase)
                .collect();
            if !parent.is_empty() {
                parent.push('/');
            }
            parent.push_str(&alias);
            if let Some((original, previous_directory)) = self.nodes.get(&parent) {
                if original != part || *previous_directory != is_directory {
                    return Err(invalid(
                        "MRPACK has conflicting file/directory paths or case aliases",
                    )
                    .into());
                }
            } else {
                self.nodes
                    .insert(parent.clone(), (part.to_string(), is_directory));
            }
            if last
                && let Some(layer) = layer
                && !self.explicit.insert((parent.clone(), layer))
            {
                return Err(invalid("Duplicate MRPACK path within one layer").into());
            }
        }
        Ok(())
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::{Cursor, Write};
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::SimpleFileOptions;

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn index() -> Value {
        json!({"formatVersion":1,"game":"minecraft","versionId":"synthetic-1",
            "name":"Original parser fixture","dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16.14"},
            "files":[]})
    }

    fn file(path: &str) -> Value {
        json!({"path":path,"hashes":{"sha1":"a".repeat(40),"sha512":"b".repeat(128)},
            "downloads":["https://cdn.modrinth.com/data/synthetic.jar"],"fileSize":4})
    }

    fn bytes(index: &Value, entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.start_file("modrinth.index.json", options).unwrap();
        writer
            .write_all(&serde_json::to_vec(index).unwrap())
            .unwrap();
        for (name, content) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn parse_bytes(bytes: &[u8]) -> Result<Mrpack, BoxError> {
        let path = std::env::temp_dir().join(format!(
            "cobble-mrpack-parser-{}-{}.mrpack",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        let result = read_mrpack(&path);
        std::fs::remove_file(path).unwrap();
        result
    }

    #[test]
    fn client_environment_and_override_layers() {
        let mut index = index();
        let required = file("mods/required.jar");
        let mut optional = file("mods/optional.jar");
        optional["env"] = json!({"client":"optional","server":"required"});
        let mut server = file("mods/server.jar");
        server["env"] = json!({"client":"unsupported","server":"required"});
        index["files"] = json!([required, optional, server]);
        let pack = parse_bytes(&bytes(
            &index,
            &[
                ("client-overrides/options.txt", b"client"),
                ("overrides/options.txt", b"common"),
                ("server-overrides/server.txt", b"server"),
            ],
        ))
        .unwrap();
        assert_eq!(pack.minecraft_version, "1.21.1");
        assert_eq!(pack.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(pack.loader_kind.as_deref(), Some("fabric"));
        assert_eq!(pack.files.len(), 2);
        assert!(!pack.files[0].optional);
        assert!(pack.files[1].optional);
        assert_eq!(pack.overrides.len(), 2);
        assert_eq!(
            (pack.overrides[0].layer, pack.overrides[0].entry_index),
            (0, 2)
        );
        assert_eq!(
            (pack.overrides[1].layer, pack.overrides[1].entry_index),
            (1, 1)
        );
    }

    #[test]
    fn exact_file_override_is_allowed_but_aliases_and_prefixes_are_rejected() {
        let mut index = index();
        index["files"] = json!([file("config/a.txt")]);
        assert!(parse_bytes(&bytes(&index, &[("overrides/config/a.txt", b"yes")])).is_ok());
        for name in ["overrides/Config/a.txt", "overrides/config/a.txt/child"] {
            assert!(parse_bytes(&bytes(&index, &[(name, b"bad")])).is_err());
        }
        index["files"] = json!([file("mods/a.jar"), file("mods/a.jar")]);
        assert!(parse_bytes(&bytes(&index, &[])).is_err());
    }

    #[test]
    fn rejects_unsafe_or_protected_paths_in_index_and_zip() {
        for name in [
            "../escape",
            "/absolute",
            "C:/absolute",
            "mods\\escape",
            "mods/CON.jar",
            "assets/a",
            "versions/a",
            ".cobble-instance.json",
            "launcher_accounts.json",
        ] {
            let mut index = index();
            index["files"] = json!([file(name)]);
            assert!(parse_bytes(&bytes(&index, &[])).is_err(), "{name}");
            assert!(
                parse_bytes(&bytes(
                    &self::index(),
                    &[(&format!("overrides/{name}"), b"bad")]
                ))
                .is_err(),
                "{name}"
            );
        }
        assert!(
            parse_bytes(&bytes(
                &index(),
                &[("overrides/config/A", b"a"), ("overrides/config/a", b"a")]
            ))
            .is_err()
        );
    }

    #[test]
    fn validates_dependencies_hashes_environment_and_urls() {
        for dependency in ["quilt-loader", "future-loader"] {
            let mut index = index();
            index["dependencies"][dependency] = json!("1");
            assert!(
                parse_bytes(&bytes(&index, &[]))
                    .unwrap_err()
                    .to_string()
                    .contains("Unsupported MRPACK dependency")
            );
        }
        for url in [
            "http://cdn.modrinth.com/a",
            "https://user:secret@github.com/a",
            "https://localhost/a",
            "https://cdn.modrinth.com.evil.invalid/a",
            "https://github.com/a b",
        ] {
            let mut index = index();
            let mut file = file("mods/a.jar");
            file["downloads"] = json!([url]);
            index["files"] = json!([file]);
            assert!(parse_bytes(&bytes(&index, &[])).is_err(), "{url}");
        }
        let mut bad = index();
        let mut entry = file("mods/a.jar");
        entry["hashes"]["sha512"] = json!("z".repeat(128));
        bad["files"] = json!([entry]);
        assert!(parse_bytes(&bytes(&bad, &[])).is_err());
        bad["formatVersion"] = json!(2);
        assert!(parse_bytes(&bytes(&bad, &[])).is_err());
    }

    #[test]
    fn vanilla_and_supported_loaders_preserve_exact_kind_and_version() {
        let mut vanilla = index();
        vanilla["dependencies"] = json!({"minecraft":"1.21.1"});
        let parsed = parse_bytes(&bytes(&vanilla, &[])).unwrap();
        assert!(parsed.loader_kind.is_none());
        assert!(parsed.loader_version.is_none());
        for (dependency, kind, version) in [
            ("fabric-loader", "fabric", "0.16.14"),
            ("forge", "forge", "52.1.14"),
            ("neoforge", "neoforge", "21.1.264"),
        ] {
            let mut fixture = vanilla.clone();
            fixture["dependencies"][dependency] = json!(version);
            let parsed = parse_bytes(&bytes(&fixture, &[])).unwrap();
            assert_eq!(parsed.loader_kind.as_deref(), Some(kind));
            assert_eq!(parsed.loader_version.as_deref(), Some(version));
            fixture["dependencies"]["minecraft"] = json!("1.20.1");
            let result = parse_bytes(&bytes(&fixture, &[]));
            if kind == "fabric" {
                assert!(result.is_ok());
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("Minecraft 1.21.1 only")
                );
            }
        }
    }

    #[test]
    fn rejects_every_pair_of_conflicting_loaders() {
        for (first, second) in [
            ("fabric-loader", "forge"),
            ("fabric-loader", "neoforge"),
            ("forge", "neoforge"),
        ] {
            let mut fixture = index();
            fixture["dependencies"] = json!({"minecraft":"1.21.1"});
            fixture["dependencies"][first] = json!("1");
            fixture["dependencies"][second] = json!("2");
            assert!(
                parse_bytes(&bytes(&fixture, &[]))
                    .unwrap_err()
                    .to_string()
                    .contains("Conflicting MRPACK loader dependencies")
            );
        }
    }

    #[test]
    fn rejects_duplicate_index_headers_and_symlinks() {
        // Rename an equal-length ordinary entry into a second index in both
        // headers; this constructs an original malformed ZIP fixture.
        let mut duplicate = bytes(&index(), &[("duplicate.index.txt", b"{}")]);
        let from = b"duplicate.index.txt";
        let to = b"modrinth.index.json";
        assert_eq!(from.len(), to.len());
        for offset in 0..duplicate.len().saturating_sub(from.len()) {
            if &duplicate[offset..offset + from.len()] == from {
                duplicate[offset..offset + from.len()].copy_from_slice(to);
            }
        }
        assert!(parse_bytes(&duplicate).is_err());
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("modrinth.index.json", SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(&serde_json::to_vec(&index()).unwrap())
            .unwrap();
        writer
            .add_symlink(
                "overrides/config/link",
                "../../escape",
                SimpleFileOptions::default(),
            )
            .unwrap();
        assert!(parse_bytes(&writer.finish().unwrap().into_inner()).is_err());
    }
}

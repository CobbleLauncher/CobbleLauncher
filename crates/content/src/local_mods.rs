// SPDX-License-Identifier: MIT
//! Descriptive scanning only: metadata does not establish compatibility or trust.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LocalMod {
    pub file_name: String,
    pub name: String,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub loader: Option<String>,
    pub enabled: bool,
    pub description: Option<String>,
}

/// Read only direct `.jar` and `.jar.disabled` files under the instance's mods directory.
/// Unknown or unreadable archives remain visible as local files with no loader metadata.
pub fn list_mods(directory: &Path) -> Result<Vec<LocalMod>, BoxError> {
    let mods = mods_directory(directory)?;
    let entries = match fs::read_dir(mods) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry?;
        // Do not follow symlinks into other directories or inspect non-regular files.
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if !is_mod_name(&file_name) {
            continue;
        }
        let enabled = !file_name.to_ascii_lowercase().ends_with(".disabled");
        let name = base_name(&file_name).to_owned();
        let mut item = LocalMod {
            file_name,
            name,
            version: None,
            authors: Vec::new(),
            loader: None,
            enabled,
            description: None,
        };
        read_metadata(&entry.path(), &mut item);
        result.push(item);
    }
    result.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(result)
}

/// Copy user-selected local JARs. All names and collisions are checked before copying.
/// Disk errors can still interrupt a batch; this API does not claim a multi-file transaction.
pub fn import_mods(directory: &Path, files: &[PathBuf]) -> Result<Vec<LocalMod>, BoxError> {
    let mods = mods_directory(directory)?;
    let mut names = HashSet::new();
    let mut imports = Vec::new();
    for source in files {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Mod filename must be UTF-8"))?;
        validate_name(name)?;
        if !name.to_ascii_lowercase().ends_with(".jar") {
            return Err(invalid("Only .jar files can be imported").into());
        }
        if !fs::symlink_metadata(source)?.file_type().is_file() {
            return Err(invalid("Import source must be a regular file").into());
        }
        if !names.insert(name.to_lowercase()) {
            return Err(invalid("Duplicate filename in selected files").into());
        }
        ensure_absent(&mods.join(name))?;
        ensure_absent(&mods.join(format!("{name}.disabled")))?;
        imports.push((source, mods.join(name)));
    }
    if !imports.is_empty() {
        fs::create_dir_all(&mods)?;
    }
    for (source, destination) in imports {
        let mut input = File::open(source)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)?;
        if let Err(error) = io::copy(&mut input, &mut output).and_then(|_| output.sync_all()) {
            drop(output);
            let _ = fs::remove_file(destination);
            return Err(error.into());
        }
    }
    list_mods(directory)
}

pub fn set_mod_enabled(
    directory: &Path,
    file_name: &str,
    enabled: bool,
) -> Result<Vec<LocalMod>, BoxError> {
    validate_name(file_name)?;
    let mods = mods_directory(directory)?;
    let source = mods.join(file_name);
    require_regular_file(&source)?;
    let currently_enabled = !file_name.to_ascii_lowercase().ends_with(".disabled");
    if currently_enabled != enabled {
        let destination_name = if enabled {
            file_name[..file_name.len() - ".disabled".len()].to_owned()
        } else {
            format!("{file_name}.disabled")
        };
        let destination = mods.join(destination_name);
        ensure_absent(&destination)?;
        fs::rename(source, destination)?;
    }
    list_mods(directory)
}

pub fn remove_mod(directory: &Path, file_name: &str) -> Result<Vec<LocalMod>, BoxError> {
    validate_name(file_name)?;
    let path = mods_directory(directory)?.join(file_name);
    require_regular_file(&path)?;
    fs::remove_file(path)?;
    list_mods(directory)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn mods_directory(directory: &Path) -> io::Result<PathBuf> {
    let mods = directory.join("mods");
    match fs::symlink_metadata(&mods) {
        Ok(metadata) if !metadata.file_type().is_dir() => {
            Err(invalid("mods must be a regular directory, not a link"))
        }
        Ok(_) => Ok(mods),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(mods),
        Err(error) => Err(error),
    }
}

fn validate_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.starts_with('.')
        || name.ends_with([' ', '.'])
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        || !is_mod_name(name)
    {
        return Err(invalid("Expected a .jar or .jar.disabled basename"));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9')
            })
        })
    {
        return Err(invalid("Reserved device filename"));
    }
    Ok(())
}

fn is_mod_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".jar") || lower.ends_with(".jar.disabled")
}

fn base_name(name: &str) -> &str {
    let suffix = if name.to_ascii_lowercase().ends_with(".disabled") {
        13
    } else {
        4
    };
    &name[..name.len() - suffix]
}

fn ensure_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "A mod with that filename already exists",
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn require_regular_file(path: &Path) -> io::Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_file() {
        Ok(())
    } else {
        Err(invalid("Mod must be a regular file, not a link"))
    }
}

fn read_metadata(path: &Path, item: &mut LocalMod) {
    let Ok(file) = File::open(path) else { return };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return;
    };
    for (name, loader) in [
        ("fabric.mod.json", "Fabric"),
        ("quilt.mod.json", "Quilt"),
        ("META-INF/neoforge.mods.toml", "NeoForge"),
        ("META-INF/mods.toml", "Forge"),
    ] {
        let Ok(entry) = archive.by_name(name) else {
            continue;
        };
        if entry.size() > MAX_METADATA_BYTES {
            continue;
        }
        let mut text = String::new();
        if entry
            .take(MAX_METADATA_BYTES + 1)
            .read_to_string(&mut text)
            .is_err()
            || text.len() as u64 > MAX_METADATA_BYTES
        {
            continue;
        }
        let metadata = if name.ends_with(".json") {
            json_metadata(&text, loader)
        } else {
            toml_metadata(&text)
        };
        if let Some(metadata) = metadata {
            item.name = metadata.name.unwrap_or_else(|| item.name.clone());
            item.version = metadata.version;
            item.authors = metadata.authors;
            item.description = metadata.description;
            item.loader = Some(loader.to_owned());
            return;
        }
    }
}

struct Metadata {
    name: Option<String>,
    version: Option<String>,
    authors: Vec<String>,
    description: Option<String>,
}

fn string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn json_metadata(text: &str, loader: &str) -> Option<Metadata> {
    let root: Value = serde_json::from_str(text).ok()?;
    let body = if loader == "Quilt" {
        root.get("quilt_loader")?
    } else {
        &root
    };
    string(body.get("id"))?;
    let display = if loader == "Quilt" {
        body.get("metadata")?
    } else {
        body
    };
    let authors = if loader == "Quilt" {
        display
            .get("contributors")
            .and_then(Value::as_object)
            .map(|people| people.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        display
            .get("authors")
            .and_then(Value::as_array)
            .map(|people| {
                people
                    .iter()
                    .filter_map(|person| {
                        string(Some(person)).or_else(|| string(person.get("name")))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(Metadata {
        name: string(display.get("name")),
        version: string(body.get("version")),
        authors,
        description: string(display.get("description")),
    })
}

fn toml_metadata(text: &str) -> Option<Metadata> {
    let root: toml::Table = text.parse().ok()?;
    // One row per physical JAR; the first mod supplies its display description.
    let first = root.get("mods")?.as_array()?.first()?.as_table()?;
    first.get("modId")?.as_str()?;
    let field = |key: &str| {
        first
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    Some(Metadata {
        name: field("displayName"),
        version: field("version").filter(|value| !value.contains("${")),
        authors: field("authors")
            .map(|authors| {
                authors
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        description: field("description"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "cobble-local-mods-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("mods")).unwrap();
            Self(path)
        }
        fn jar(&self, file_name: &str, metadata_name: &str, text: &str) {
            let file = File::create(self.0.join("mods").join(file_name)).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer
                .start_file(
                    metadata_name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .unwrap();
            writer.write_all(text.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn metadata_is_descriptive_and_corrupt_jars_remain_visible() {
        let fixture = Fixture::new();
        fixture.jar("fabric.jar", "fabric.mod.json", r#"{"id":"synthetic","name":"Synthetic Fabric","version":"1.2","authors":["Alice",{"name":"Bob"}],"description":"Original fixture"}"#);
        fixture.jar("quilt.jar.disabled", "quilt.mod.json", r#"{"quilt_loader":{"id":"synthetic","version":"2","metadata":{"name":"Synthetic Quilt","contributors":{"Carol":"Developer"}}}}"#);
        fixture.jar("unknown.jar", "fabric.mod.json", "malformed JSON");
        fs::write(fixture.0.join("mods/broken.jar"), b"not a zip").unwrap();
        fs::write(fixture.0.join("mods/ignored.txt"), b"ignored").unwrap();
        let mods = list_mods(&fixture.0).unwrap();
        assert_eq!(mods.len(), 4);
        assert_eq!(mods[0].name, "broken");
        assert!(mods[0].loader.is_none());
        assert_eq!(mods[1].name, "Synthetic Fabric");
        assert_eq!(mods[1].authors, ["Alice", "Bob"]);
        assert_eq!(mods[1].version.as_deref(), Some("1.2"));
        assert_eq!(mods[2].loader.as_deref(), Some("Quilt"));
        assert!(!mods[2].enabled);
        assert_eq!(mods[2].authors, ["Carol"]);
        assert!(mods[3].loader.is_none());
    }

    #[test]
    fn parses_forge_and_neoforge_without_claiming_manifest_version_substitution() {
        let fixture = Fixture::new();
        let text = "modLoader='javafml'\n[[mods]]\nmodId='synthetic'\ndisplayName='Synthetic Forge'\nversion='${file.jarVersion}'\nauthors='Alice, Bob'\ndescription='Fixture'\n";
        fixture.jar("forge.jar", "META-INF/mods.toml", text);
        fixture.jar(
            "neo.jar",
            "META-INF/neoforge.mods.toml",
            &text.replace("${file.jarVersion}", "3.0"),
        );
        let mods = list_mods(&fixture.0).unwrap();
        assert_eq!(mods[0].loader.as_deref(), Some("Forge"));
        assert_eq!(mods[0].name, "Synthetic Forge");
        assert_eq!(mods[0].authors, ["Alice", "Bob"]);
        assert!(mods[0].version.is_none());
        assert_eq!(mods[1].loader.as_deref(), Some("NeoForge"));
        assert_eq!(mods[1].version.as_deref(), Some("3.0"));
    }

    #[test]
    fn renames_disable_enable_and_removes_actual_file() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("mods/local.jar"), b"local mod bytes").unwrap();
        let mods = set_mod_enabled(&fixture.0, "local.jar", false).unwrap();
        assert_eq!(mods[0].file_name, "local.jar.disabled");
        assert!(!mods[0].enabled);
        assert_eq!(
            fs::read(fixture.0.join("mods/local.jar.disabled")).unwrap(),
            b"local mod bytes"
        );
        assert!(set_mod_enabled(&fixture.0, "local.jar.disabled", true).unwrap()[0].enabled);
        assert!(remove_mod(&fixture.0, "local.jar").unwrap().is_empty());
    }

    #[test]
    fn imports_preflight_the_whole_batch_and_do_not_overwrite_disabled_mods() {
        let source = Fixture::new();
        let target = Fixture::new();
        fs::write(source.0.join("mods/a.jar"), b"a").unwrap();
        fs::write(source.0.join("mods/b.jar"), b"b").unwrap();
        fs::write(target.0.join("mods/b.jar.disabled"), b"old").unwrap();
        let files = [source.0.join("mods/a.jar"), source.0.join("mods/b.jar")];
        assert!(import_mods(&target.0, &files).is_err());
        assert!(!target.0.join("mods/a.jar").exists());
        fs::remove_file(target.0.join("mods/b.jar.disabled")).unwrap();
        assert_eq!(import_mods(&target.0, &files).unwrap().len(), 2);
        assert!(import_mods(&target.0, &files).is_err());
        assert_eq!(fs::read(target.0.join("mods/b.jar")).unwrap(), b"b");
        assert!(import_mods(&source.0, &[files[0].clone(), files[0].clone()]).is_err());
    }

    #[test]
    fn rejects_paths_and_refuses_toggle_collisions() {
        let fixture = Fixture::new();
        for name in [
            "../other.jar",
            "..\\other.jar",
            "C:other.jar",
            "/other.jar",
            "CON.jar",
            "mods/other.jar",
            "file.txt",
        ] {
            assert!(remove_mod(&fixture.0, name).is_err(), "{name}");
            assert!(set_mod_enabled(&fixture.0, name, false).is_err(), "{name}");
        }
        fs::write(fixture.0.join("mods/a.jar"), b"a").unwrap();
        fs::write(fixture.0.join("mods/a.jar.disabled"), b"disabled").unwrap();
        assert!(set_mod_enabled(&fixture.0, "a.jar", false).is_err());
        assert_eq!(
            fs::read(fixture.0.join("mods/a.jar.disabled")).unwrap(),
            b"disabled"
        );
    }

    #[test]
    fn oversized_metadata_is_unknown_and_missing_mods_is_empty() {
        let fixture = Fixture::new();
        fixture.jar(
            "large.jar",
            "fabric.mod.json",
            &" ".repeat(MAX_METADATA_BYTES as usize + 1),
        );
        assert!(list_mods(&fixture.0).unwrap()[0].loader.is_none());
        fs::remove_dir_all(fixture.0.join("mods")).unwrap();
        assert!(list_mods(&fixture.0).unwrap().is_empty());
    }
}

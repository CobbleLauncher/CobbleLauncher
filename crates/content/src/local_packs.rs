// SPDX-License-Identifier: MIT
//! Local ZIP files only. Pack selection stays in Minecraft or its shader Mod.
use crate::BoxError;
use cobble_store::SafeRelativePath;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
use unicode_normalization::UnicodeNormalization;

const MAX_METADATA: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalPackKind {
    ResourcePack,
    ShaderPack,
}

impl LocalPackKind {
    fn folder(self) -> &'static str {
        match self {
            Self::ResourcePack => "resourcepacks",
            Self::ShaderPack => "shaderpacks",
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LocalPack {
    pub file_name: String,
    pub name: String,
    pub description: Option<String>,
    pub size_bytes: u64,
    pub pack_format: Option<u32>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn linked(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn directory(instance: &Path, kind: LocalPackKind) -> Result<PathBuf, BoxError> {
    let root = fs::symlink_metadata(instance)?;
    if linked(&root) || !root.is_dir() {
        return Err(invalid("Game must be a real directory").into());
    }
    let directory = instance.join(kind.folder());
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if linked(&metadata) || !metadata.is_dir() => {
            return Err(invalid("Pack directory must be a real directory, not a link").into());
        }
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    Ok(directory)
}

fn zip_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".zip")
}

fn validate_name(name: &str) -> Result<(), BoxError> {
    let safe = SafeRelativePath::parse(name)?;
    if safe.as_str().contains('/') || name.starts_with('.') || !zip_name(name) {
        return Err(invalid("Expected a ZIP basename").into());
    }
    Ok(())
}

fn collision_key(name: &str) -> String {
    caseless::default_case_fold_str(&name.nfkc().collect::<String>())
        .chars()
        .flat_map(char::to_uppercase)
        .collect()
}

/// Existing unknown or unreadable ZIPs stay visible. Directories and links are
/// excluded, and this operation does not claim that a pack is selected or compatible.
pub fn list_local_packs(instance: &Path, kind: LocalPackKind) -> Result<Vec<LocalPack>, BoxError> {
    let directory = directory(instance, kind)?;
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut packs = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !zip_name(&name) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if linked(&metadata) || !metadata.is_file() {
            continue;
        }
        let (description, pack_format) = File::open(entry.path())
            .ok()
            .and_then(|mut file| inspect(&mut file, kind).ok())
            .unwrap_or_default();
        packs.push(LocalPack {
            name: if name.len() == 4 {
                name.clone()
            } else {
                name[..name.len() - 4].to_owned()
            },
            file_name: name,
            description,
            size_bytes: metadata.len(),
            pack_format,
        });
    }
    packs.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(packs)
}

/// Validate the complete selection before copying any file. A disk failure can
/// leave earlier successful imports; only the current owned partial is removed.
pub fn import_local_packs(
    instance: &Path,
    kind: LocalPackKind,
    files: &[PathBuf],
) -> Result<Vec<LocalPack>, BoxError> {
    let directory = directory(instance, kind)?;
    let mut names = HashSet::new();
    if directory.exists() {
        for entry in fs::read_dir(&directory)? {
            names.insert(collision_key(&entry?.file_name().to_string_lossy()));
        }
    }
    let mut imports = Vec::new();
    for source in files {
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| invalid("Pack filename must be UTF-8"))?;
        validate_name(name)?;
        if !names.insert(collision_key(name)) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "A pack with that filename already exists",
            )
            .into());
        }
        let metadata = fs::symlink_metadata(source)?;
        if linked(&metadata) || !metadata.is_file() {
            return Err(invalid("Pack source must be a regular file, not a link").into());
        }
        let mut input = File::open(source)?;
        inspect(&mut input, kind)?;
        input.seek(SeekFrom::Start(0))?;
        imports.push((input, directory.join(name)));
    }
    if !imports.is_empty() {
        fs::create_dir_all(&directory)?;
    }
    for (mut input, destination) in imports {
        copy_owned(&mut input, &destination)?;
    }
    list_local_packs(instance, kind)
}

fn copy_owned(input: &mut impl Read, destination: &Path) -> io::Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = io::copy(input, &mut output).and_then(|_| output.sync_all());
    drop(output);
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result.map(|_| ())
}

pub fn remove_local_pack(
    instance: &Path,
    kind: LocalPackKind,
    file_name: &str,
) -> Result<Vec<LocalPack>, BoxError> {
    validate_name(file_name)?;
    let target = directory(instance, kind)?.join(file_name);
    let metadata = fs::symlink_metadata(&target)?;
    if linked(&metadata) || !metadata.is_file() {
        return Err(invalid("Pack must be a regular ZIP file, not a folder or link").into());
    }
    fs::remove_file(target)?;
    list_local_packs(instance, kind)
}

fn inspect(
    file: &mut File,
    kind: LocalPackKind,
) -> Result<(Option<String>, Option<u32>), BoxError> {
    let mut archive = zip::ZipArchive::new(file)?;
    if kind == LocalPackKind::ShaderPack {
        let has_shaders = archive.file_names().any(|name| {
            name.is_ok_and(|name| name.starts_with("shaders/") && !name.ends_with('/'))
        });
        if !has_shaders {
            return Err(
                invalid("Shader ZIP must contain files in its root shaders directory").into(),
            );
        }
        return Ok((None, None));
    }
    let entry = archive
        .by_name("pack.mcmeta")
        .map_err(|_| invalid("Resource ZIP must contain root pack.mcmeta"))?;
    if entry.is_dir() || entry.size() > MAX_METADATA {
        return Err(invalid("Resource pack metadata exceeds 1 MiB or is not a file").into());
    }
    let mut bytes = Vec::new();
    entry.take(MAX_METADATA + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(invalid("Resource pack metadata exceeds 1 MiB").into());
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    let pack = value
        .get("pack")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Resource pack metadata must contain a pack object"))?;
    let pack_format = pack
        .get("pack_format")
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok());
    let mut description = String::new();
    if let Some(value) = pack.get("description") {
        flatten_description(value, &mut description, 0);
    }
    Ok((
        if description.is_empty() {
            None
        } else {
            Some(description)
        },
        pack_format,
    ))
}

fn flatten_description(value: &Value, text: &mut String, depth: usize) {
    if depth > 16 || text.chars().count() >= 4096 {
        return;
    }
    match value {
        Value::String(value) => text.extend(value.chars().take(4096 - text.chars().count())),
        Value::Array(values) => {
            for value in values {
                flatten_description(value, text, depth + 1);
            }
        }
        Value::Object(value) => {
            if let Some(value) = value.get("text") {
                flatten_description(value, text, depth + 1);
            }
            if let Some(value) = value.get("extra") {
                flatten_description(value, text, depth + 1);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "cobble-localpacks-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn zip(&self, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
            let path = self.0.join(name);
            let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
            for (name, bytes) in entries {
                archive
                    .start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                archive.write_all(bytes).unwrap();
            }
            archive.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    const META:&[u8]=br#"{"pack":{"pack_format":34,"description":["Hello ",{"text":"world","extra":[{"text":"!"}]}]}}"#;

    #[test]
    fn resource_import_keeps_bytes_reads_text_components_and_remove_preserves_foreign_files() {
        let sources = Fixture::new();
        let instance = Fixture::new();
        let source = sources.zip(
            "My textures.zip",
            &[
                ("pack.mcmeta", META),
                ("assets/demo/texture.png", b"synthetic"),
            ],
        );
        let packs = import_local_packs(
            &instance.0,
            LocalPackKind::ResourcePack,
            std::slice::from_ref(&source),
        )
        .unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "My textures");
        assert_eq!(packs[0].description.as_deref(), Some("Hello world!"));
        assert_eq!(packs[0].pack_format, Some(34));
        assert_eq!(
            fs::read(instance.0.join("resourcepacks/My textures.zip")).unwrap(),
            fs::read(&source).unwrap()
        );
        assert!(!instance.0.join("resourcepacks/assets").exists());
        fs::write(instance.0.join("resourcepacks/keep.txt"), b"foreign").unwrap();
        fs::create_dir(instance.0.join("resourcepacks/folder.zip")).unwrap();
        assert!(remove_local_pack(&instance.0, LocalPackKind::ResourcePack, "folder.zip").is_err());
        assert!(
            remove_local_pack(&instance.0, LocalPackKind::ResourcePack, "../keep.txt").is_err()
        );
        assert!(
            remove_local_pack(&instance.0, LocalPackKind::ResourcePack, "My textures.zip")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            fs::read(instance.0.join("resourcepacks/keep.txt")).unwrap(),
            b"foreign"
        );
        assert!(source.exists());
    }

    #[test]
    fn whole_selection_rejects_wrong_kind_nested_metadata_and_collisions_before_any_copy() {
        let sources = Fixture::new();
        let instance = Fixture::new();
        let resource = sources.zip("resource.zip", &[("pack.mcmeta", META)]);
        let shader = sources.zip("shader.zip", &[("shaders/main.fsh", b"synthetic shader")]);
        assert!(
            import_local_packs(
                &instance.0,
                LocalPackKind::ResourcePack,
                &[resource.clone(), shader.clone()]
            )
            .is_err()
        );
        assert!(!instance.0.join("resourcepacks").exists());
        let nested = sources.zip("nested.zip", &[("nested/pack.mcmeta", META)]);
        assert!(import_local_packs(&instance.0, LocalPackKind::ResourcePack, &[nested]).is_err());
        assert!(import_local_packs(&instance.0, LocalPackKind::ShaderPack, &[resource]).is_err());
        assert_eq!(
            import_local_packs(&instance.0, LocalPackKind::ShaderPack, &[shader])
                .unwrap()
                .len(),
            1
        );
        let collision = sources.zip("SHADER.ZIP", &[("shaders/main.fsh", b"different")]);
        assert!(import_local_packs(&instance.0, LocalPackKind::ShaderPack, &[collision]).is_err());
        let packs = list_local_packs(&instance.0, LocalPackKind::ShaderPack).unwrap();
        assert_eq!(packs[0].file_name, "shader.zip");
    }

    #[test]
    fn unknown_zip_stays_visible_and_import_copies_internal_paths_without_extraction() {
        let sources = Fixture::new();
        let instance = Fixture::new();
        fs::create_dir(instance.0.join("resourcepacks")).unwrap();
        fs::write(instance.0.join("resourcepacks/broken.zip"), b"not a ZIP").unwrap();
        let packs = list_local_packs(&instance.0, LocalPackKind::ResourcePack).unwrap();
        assert_eq!(packs[0].name, "broken");
        assert_eq!(packs[0].description, None);
        let unsafe_zip = sources.zip(
            "escape.zip",
            &[("pack.mcmeta", META), ("../escape", b"bad")],
        );
        import_local_packs(
            &instance.0,
            LocalPackKind::ResourcePack,
            std::slice::from_ref(&unsafe_zip),
        )
        .unwrap();
        assert_eq!(
            fs::read(instance.0.join("resourcepacks/escape.zip")).unwrap(),
            fs::read(&unsafe_zip).unwrap()
        );
        let large = vec![b' '; MAX_METADATA as usize + 1];
        let oversized = sources.zip("large.zip", &[("pack.mcmeta", &large)]);
        assert!(
            import_local_packs(&instance.0, LocalPackKind::ResourcePack, &[oversized]).is_err()
        );
        let invalid = sources.zip("invalid.zip", &[("pack.mcmeta", br#"{"other":{}}"#)]);
        assert!(import_local_packs(&instance.0, LocalPackKind::ResourcePack, &[invalid]).is_err());
        assert_eq!(
            fs::read(instance.0.join("resourcepacks/broken.zip")).unwrap(),
            b"not a ZIP"
        );
        assert!(!instance.0.join("escape").exists());
    }

    #[test]
    fn failed_copy_removes_only_its_owned_partial_and_never_overwrites_existing_file() {
        struct Broken {
            read: bool,
        }
        impl Read for Broken {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                if self.read {
                    Err(io::Error::other("synthetic read failure"))
                } else {
                    self.read = true;
                    bytes[0] = 42;
                    Ok(1)
                }
            }
        }
        let instance = Fixture::new();
        let target = instance.0.join("partial.zip");
        assert!(copy_owned(&mut Broken { read: false }, &target).is_err());
        assert!(!target.exists());
        fs::write(&target, b"keep").unwrap();
        assert!(copy_owned(&mut io::Cursor::new(b"replace"), &target).is_err());
        assert_eq!(fs::read(target).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn links_are_neither_imported_listed_nor_removed() {
        let sources = Fixture::new();
        let instance = Fixture::new();
        let source = sources.zip("resource.zip", &[("pack.mcmeta", META)]);
        fs::create_dir(instance.0.join("resourcepacks")).unwrap();
        let link = instance.0.join("resourcepacks/link.zip");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(
            list_local_packs(&instance.0, LocalPackKind::ResourcePack)
                .unwrap()
                .is_empty()
        );
        assert!(remove_local_pack(&instance.0, LocalPackKind::ResourcePack, "link.zip").is_err());
        assert!(import_local_packs(&instance.0, LocalPackKind::ResourcePack, &[link]).is_err());
        assert!(source.exists());
        fs::remove_file(instance.0.join("resourcepacks/link.zip")).unwrap();
        fs::remove_dir(instance.0.join("resourcepacks")).unwrap();
        std::os::unix::fs::symlink(&sources.0, instance.0.join("resourcepacks")).unwrap();
        assert!(list_local_packs(&instance.0, LocalPackKind::ResourcePack).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_pack_directory_junction_is_rejected_without_touching_foreign_content() {
        use std::os::windows::process::CommandExt;
        let sources = Fixture::new();
        let instance = Fixture::new();
        let source = sources.zip("resource.zip", &[("pack.mcmeta", META)]);
        let junction = instance.0.join("resourcepacks");
        let result = std::process::Command::new("cmd")
            .args(["/D", "/C"])
            .raw_arg(format!(
                "mklink /J \"{}\" \"{}\"",
                junction.display(),
                sources.0.display()
            ))
            .creation_flags(0x0800_0000)
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(list_local_packs(&instance.0, LocalPackKind::ResourcePack).is_err());
        assert!(
            import_local_packs(
                &instance.0,
                LocalPackKind::ResourcePack,
                std::slice::from_ref(&source)
            )
            .is_err()
        );
        assert!(
            remove_local_pack(&instance.0, LocalPackKind::ResourcePack, "resource.zip").is_err()
        );
        assert!(source.exists());
        fs::remove_dir(junction).unwrap();
    }
}

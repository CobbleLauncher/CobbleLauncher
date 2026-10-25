// SPDX-License-Identifier: MIT
//! Host-selected ZIP/JSON snapshots. Only language entries are copied, never extracted to disk.
use super::{Document, FileInfo, SelectionInfo};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_TEXT: usize = 1024 * 1024;
const MAX_TOTAL: usize = 4 * 1024 * 1024;
pub(super) struct Source {
    pub info: SelectionInfo,
    pub files: Vec<Document>,
}
fn code(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && value.len() <= 64
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_.-".contains(&c))
}
fn read_text(mut input: impl Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(MAX_TEXT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "LANG_PACKAGE_IO")?;
    if bytes.len() > MAX_TEXT {
        return Err("LANG_PACKAGE_LIMIT".into());
    }
    String::from_utf8(bytes).map_err(|_| "LANG_PACKAGE_NOT_UTF8".into())
}
fn regular(path: &Path) -> Result<fs::File, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "LANG_PACKAGE_IO")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("LANG_PACKAGE_INVALID_FILE".into());
        }
    }
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("LANG_PACKAGE_INVALID_FILE".into());
    }
    fs::File::open(path).map_err(|_| "LANG_PACKAGE_IO".into())
}
pub(super) fn read(paths: Vec<PathBuf>) -> Result<Source, String> {
    if paths.is_empty() || paths.len() > 2 {
        return Err("LANG_PACKAGE_SELECTION_INVALID".into());
    }
    let name = paths[0]
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("LANG_PACKAGE_INVALID_FILE")?
        .to_owned();
    if name.len() > 256 || name.chars().any(char::is_control) {
        return Err("LANG_PACKAGE_INVALID_FILE".into());
    }
    let mut source = Source {
        info: SelectionInfo {
            selection_id: String::new(),
            source_name: name,
            kind: "language_files".into(),
            pack_format: None,
            files: vec![],
        },
        files: vec![],
    };
    let zip = paths[0]
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"));
    if zip {
        if paths.len() != 1 {
            return Err("LANG_PACKAGE_SELECTION_INVALID".into());
        }
        source.info.kind = "resource_pack".into();
        let file = regular(&paths[0])?;
        if file.metadata().map_err(|_| "LANG_PACKAGE_IO")?.len() > 64 * 1024 * 1024 {
            return Err("LANG_PACKAGE_LIMIT".into());
        }
        let mut archive = zip::ZipArchive::new(file).map_err(|_| "LANG_PACKAGE_INVALID_ZIP")?;
        if archive.len() > 10_000 {
            return Err("LANG_PACKAGE_LIMIT".into());
        }
        let mut names = BTreeSet::new();
        let mut total = 0;
        for index in 0..archive.len() {
            let entry = archive
                .by_index(index)
                .map_err(|_| "LANG_PACKAGE_INVALID_ZIP")?;
            if entry.is_dir()
                || entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                continue;
            }
            let name = entry
                .name()
                .map_err(|_| "LANG_PACKAGE_INVALID_ZIP")?
                .into_owned();
            if name == "pack.mcmeta" && entry.size() <= 65536 {
                if let Ok(text) = read_text(entry) {
                    source.info.pack_format = serde_json::from_str::<serde_json::Value>(&text)
                        .ok()
                        .and_then(|v| v["pack"]["pack_format"].as_i64())
                        .and_then(|v| i32::try_from(v).ok());
                }
                continue;
            }
            let parts: Vec<_> = name.split('/').collect();
            if parts.len() != 4 || parts[0] != "assets" || parts[2] != "lang" {
                continue;
            }
            let Some(locale) = parts[3].strip_suffix(".json") else {
                continue;
            };
            if !code(parts[1]) || !code(locale) {
                return Err("LANG_PACKAGE_INVALID_ENTRY".into());
            }
            if source.files.len() >= 64 || entry.size() > MAX_TEXT as u64 {
                return Err("LANG_PACKAGE_LIMIT".into());
            }
            if !names.insert(name.clone()) {
                return Err("LANG_PACKAGE_DUPLICATE_FILE".into());
            }
            let locale = locale.to_owned();
            let namespace = parts[1].to_owned();
            let text = read_text(entry)?;
            total += text.len();
            if total > MAX_TOTAL {
                return Err("LANG_PACKAGE_LIMIT".into());
            }
            let id = format!("file-{}", source.files.len());
            source.info.files.push(FileInfo {
                id: id.clone(),
                namespace: Some(namespace),
                locale: locale.clone(),
                name: name.clone(),
            });
            source.files.push(Document {
                id,
                name,
                locale,
                text,
            });
        }
    } else {
        let mut locales = BTreeSet::new();
        for path in paths {
            if !path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("json"))
            {
                return Err("LANG_PACKAGE_SELECTION_INVALID".into());
            }
            let locale = path
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| code(s))
                .ok_or("LANG_PACKAGE_INVALID_LOCALE")?
                .to_owned();
            if !locales.insert(locale.clone()) {
                return Err("LANG_PACKAGE_DUPLICATE_FILE".into());
            }
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("LANG_PACKAGE_INVALID_FILE")?
                .to_owned();
            let text = read_text(regular(&path)?)?;
            let id = format!("file-{}", source.files.len());
            source.info.files.push(FileInfo {
                id: id.clone(),
                namespace: None,
                locale: locale.clone(),
                name: name.clone(),
            });
            source.files.push(Document {
                id,
                name,
                locale,
                text,
            });
        }
    }
    if source.files.is_empty() {
        return Err("LANG_PACKAGE_NO_LANGUAGES".into());
    }
    source
        .info
        .files
        .sort_by(|a, b| (&a.namespace, &a.locale).cmp(&(&b.namespace, &b.locale)));
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn language_zip_exposes_only_safe_lang_entries_and_exact_namespace_format() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pack.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, text) in [
            ("pack.mcmeta", r#"{"pack":{"pack_format":34}}"#),
            ("assets/demo/lang/en_us.json", r#"{"key":"Value %s"}"#),
            ("assets/demo/lang/zh_cn.json", r#"{"key":"值 %s"}"#),
            ("private/token.txt", "not exposed"),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        let source = read(vec![path]).unwrap();
        assert_eq!(source.files.len(), 2);
        assert_eq!(source.info.pack_format, Some(34));
        assert_eq!(source.info.files[0].namespace.as_deref(), Some("demo"));
        assert!(source.files.iter().all(|f| !f.text.contains("not exposed")));
    }
    #[test]
    fn loose_language_files_keep_namespace_unknown_and_do_not_accept_other_formats() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("en_us.json");
        fs::write(&path, r#"{"key":"hello"}"#).unwrap();
        let source = read(vec![path]).unwrap();
        assert!(source.info.files[0].namespace.is_none());
        assert_eq!(source.info.files[0].locale, "en_us");
        let other = root.path().join("secrets.txt");
        fs::write(&other, "not a language").unwrap();
        assert!(read(vec![other]).is_err());
    }
    #[test]
    fn language_zip_rejects_unsafe_namespace_and_oversize_language_entries() {
        for (name, text) in [
            ("assets/../lang/en_us.json", "{}".into()),
            ("assets/demo/lang/en_us.json", "x".repeat(MAX_TEXT + 1)),
        ] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("pack.zip");
            let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(text.as_bytes()).unwrap();
            zip.finish().unwrap();
            let before = fs::read(&path).unwrap();
            assert!(read(vec![path.clone()]).is_err());
            assert_eq!(fs::read(path).unwrap(), before);
        }
    }
}

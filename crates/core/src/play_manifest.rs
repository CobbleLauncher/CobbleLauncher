// SPDX-License-Identifier: MIT
//! Shareable references and explicitly selected text overrides; never embeds Mod jars.
use cap_std::fs::Dir;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::Path,
};
type Error = Box<dyn std::error::Error + Send + Sync>;
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigChoice {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub text: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModReference {
    pub name: String,
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub sha1: String,
    pub url: String,
    pub size: u64,
    pub sha512: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub revision: String,
    pub name: String,
    pub version: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub mods: Vec<ModReference>,
    pub configs: Vec<ConfigChoice>,
    pub excluded_mods: Vec<String>,
}
fn plain(path: &Path) -> Result<(), Error> {
    let m = fs::symlink_metadata(path)?;
    if m.file_type().is_symlink() {
        return Err("PLAY_SHARE_UNSAFE_FILE".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("PLAY_SHARE_UNSAFE_FILE".into());
        }
    }
    Ok(())
}
fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    plain(path)?;
    let f = fs::File::open(path)?;
    if !f.metadata()?.is_file() || f.metadata()?.len() > limit {
        return Err("PLAY_SHARE_LIMIT".into());
    }
    let mut b = Vec::new();
    f.take(limit + 1).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return Err("PLAY_SHARE_LIMIT".into());
    }
    Ok(b)
}
fn configs(root: &Path) -> Result<BTreeMap<String, (String, Vec<u8>)>, Error> {
    let mut out = BTreeMap::new();
    let base = root.join("config");
    if !base.exists() {
        return Ok(out);
    }
    plain(&base)?;
    let mut pending = vec![(base, 0)];
    let mut total = 0;
    let mut entries = 0;
    while let Some((dir, depth)) = pending.pop() {
        if depth > 8 {
            return Err("PLAY_SHARE_LIMIT".into());
        }
        plain(&dir)?;
        for item in fs::read_dir(dir)? {
            let path = item?.path();
            entries += 1;
            if entries > 4096 {
                return Err("PLAY_SHARE_LIMIT".into());
            }
            plain(&path)?;
            if path.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            if !matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("json" | "json5" | "toml" | "cfg" | "properties" | "yaml" | "yml" | "txt")
            ) {
                continue;
            }
            let data = bytes(&path, 65_536)?;
            if std::str::from_utf8(&data).is_err() {
                continue;
            }
            total += data.len();
            if out.len() >= 64 || total > 524_288 {
                return Err("PLAY_SHARE_LIMIT".into());
            }
            let name = path
                .strip_prefix(root)?
                .to_str()
                .ok_or("PLAY_SHARE_UNSAFE_FILE")?
                .replace('\\', "/");
            let id = format!("{:x}", Sha256::digest(name.as_bytes()));
            out.insert(id, (name, data));
        }
    }
    Ok(out)
}
fn mod_bytes(root: &Path) -> Result<BTreeMap<String, (String, u64, String)>, Error> {
    let base = root.join("mods");
    let mut out = BTreeMap::new();
    let mut scanned = 0u64;
    let mut entries = 0;
    if !base.exists() {
        return Ok(out);
    }
    plain(&base)?;
    for item in fs::read_dir(base)? {
        let path = item?.path();
        entries += 1;
        if entries > 4096 {
            return Err("PLAY_SHARE_LIMIT".into());
        }
        if path.extension().and_then(|s| s.to_str()) != Some("jar") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("PLAY_SHARE_UNSAFE_FILE")?
            .to_owned();
        plain(&path)?;
        let mut file = fs::File::open(&path)?;
        let size = file.metadata()?.len();
        if !file.metadata()?.is_file() || size > 536_870_912 {
            return Err("PLAY_SHARE_LIMIT".into());
        }
        scanned = scanned.checked_add(size).ok_or("PLAY_SHARE_LIMIT")?;
        if scanned > 2_147_483_648 {
            return Err("PLAY_SHARE_LIMIT".into());
        }
        let mut digest = sha1::Sha1::new();
        let mut sha512 = sha2::Sha512::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
            sha512.update(&buffer[..n]);
        }
        out.insert(
            name,
            (
                format!("{:x}", digest.finalize()),
                size,
                format!("{:x}", sha512.finalize()),
            ),
        );
        if out.len() > 1024 {
            return Err("PLAY_SHARE_LIMIT".into());
        }
    }
    Ok(out)
}
pub async fn preview(
    root: &Path,
    name: &str,
    version: &str,
    loader: Option<&str>,
    loader_version: Option<&str>,
) -> Result<Preview, Error> {
    static SCANS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
        std::sync::OnceLock::new();
    let permit = SCANS
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
        .acquire_owned()
        .await?;
    let scan_root = root.to_owned();
    let (provenance, local, choices) = tokio::task::spawn_blocking(move || -> Result<_, Error> {
        let _permit = permit;
        plain(&scan_root)?;
        let directory = Dir::open_ambient_dir(&scan_root, cap_std::ambient_authority())?;
        Ok((
            crate::modrinth::read_provenance(&directory)?,
            mod_bytes(&scan_root)?,
            configs(&scan_root)?,
        ))
    })
    .await??;
    let mut refs = Vec::new();
    let selections: Vec<_> = provenance
        .iter()
        .filter(|(_, m)| local.contains_key(&m.filename))
        .map(|(id, m)| cobble_content::ModVersionSelection {
            project_id: id.clone(),
            version_id: m.version_id.clone(),
        })
        .collect();
    if !selections.is_empty() {
        for batch in selections.chunks(64) {
            let plan = cobble_content::resolve_modrinth_selected(
                batch,
                version,
                loader.unwrap_or("fabric"),
            )
            .await?;
            for selected in plan.mods {
                let Some(managed) = provenance.get(&selected.project_id) else {
                    continue;
                };
                let Some(data) = local.get(&managed.filename) else {
                    continue;
                };
                let hash = data.0.clone();
                if hash != managed.sha1
                    || hash != selected.file.sha1
                    || selected.file.size != data.1
                    || selected
                        .file
                        .sha512
                        .as_ref()
                        .is_some_and(|hash| hash != &data.2)
                    || selected.file.filename != managed.filename
                    || !selected.file.url.starts_with("https://cdn.modrinth.com/")
                {
                    continue;
                }
                if refs
                    .iter()
                    .any(|entry: &ModReference| entry.name == managed.filename)
                {
                    continue;
                }
                refs.push(ModReference {
                    name: managed.filename.clone(),
                    source: "Modrinth".into(),
                    project_id: selected.project_id,
                    version_id: selected.version_id,
                    sha1: hash,
                    url: selected.file.url,
                    size: data.1,
                    sha512: data.2.clone(),
                });
            }
        }
    }
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&(
        name,
        version,
        loader,
        loader_version,
        &refs,
    ))?);
    for (id, (path, data)) in &choices {
        hash.update(id);
        hash.update(path);
        hash.update(data);
    }
    for (path, data) in &local {
        hash.update(path);
        hash.update(&data.0);
        hash.update(data.1.to_le_bytes());
    }
    let excluded_mods = local
        .keys()
        .filter(|n| !refs.iter().any(|r| &r.name == *n))
        .cloned()
        .collect();
    let preview = Preview {
        revision: format!("{:x}", hash.finalize()),
        name: name.into(),
        version: version.into(),
        loader: loader.map(str::to_owned),
        loader_version: loader_version.map(str::to_owned),
        mods: refs,
        configs: choices
            .into_iter()
            .map(|(id, (name, data))| ConfigChoice {
                id,
                name,
                size: data.len() as u64,
                text: String::from_utf8(data).unwrap_or_default(),
            })
            .collect(),
        excluded_mods,
    };
    if serde_json::to_vec(&preview)?.len() > 1_048_576 {
        return Err("PLAY_SHARE_LIMIT".into());
    }
    Ok(preview)
}
/// Caller holds a stopped-instance lease and owns the exact save-dialog destination.
pub fn write(
    root: &Path,
    destination: &Path,
    preview: &Preview,
    selected: &[String],
) -> Result<(), Error> {
    if preview.loader.is_some() != preview.loader_version.is_some() {
        return Err("PLAY_SHARE_UNSUPPORTED_LOADER".into());
    }
    let choices = configs(root)?;
    let unique: BTreeSet<_> = selected.iter().collect();
    if unique.len() != selected.len()
        || selected.len() > 64
        || selected.iter().any(|id| !choices.contains_key(id))
    {
        return Err("PLAY_SHARE_CONFIG_SELECTION".into());
    }
    let mut dependencies = serde_json::json!({"minecraft":preview.version});
    if let (Some(kind), Some(version)) = (&preview.loader, &preview.loader_version) {
        let key = match kind.as_str() {
            "fabric" => "fabric-loader",
            "forge" => "forge",
            "neoforge" => "neoforge",
            _ => return Err("PLAY_SHARE_UNSUPPORTED_LOADER".into()),
        };
        dependencies[key] = version.clone().into();
    }
    let files:Vec<_>=preview.mods.iter().map(|m|serde_json::json!({"path":format!("mods/{}",m.name),"hashes":{"sha1":m.sha1,"sha512":m.sha512},"downloads":[m.url],"fileSize":m.size})).collect();
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| -> Result<(), Error> {
        let mut zip = zip::ZipWriter::new(output);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options)?;
        zip.write_all(&serde_json::to_vec(&serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":"1.0.0","name":preview.name,"files":files,"dependencies":dependencies}))?)?;
        for id in selected {
            let (name, data) = &choices[id];
            if !preview
                .configs
                .iter()
                .any(|c| &c.id == id && c.text.as_bytes() == data)
            {
                return Err("PLAY_SHARE_STALE".into());
            }
            zip.start_file(format!("overrides/{name}"), options)?;
            zip.write_all(data)?;
        }
        zip.finish()?.sync_all()?;
        cobble_content::read_mrpack(destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    fn root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "cobble-play-share-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("config")).unwrap();
        root
    }
    #[test]
    fn selected_overrides_only_and_no_private_payload() {
        let root = root();
        fs::write(root.join("config/synthetic.cfg"), b"enabled=true").unwrap();
        fs::write(root.join("accounts.json"), b"SECRET").unwrap();
        fs::create_dir(root.join("mods")).unwrap();
        fs::write(root.join("mods/unknown.jar"), b"NO REDISTRIBUTION").unwrap();
        let choices = configs(&root).unwrap();
        let preview = Preview {
            revision: "test".into(),
            name: "Synthetic".into(),
            version: "1.21.1".into(),
            loader: None,
            loader_version: None,
            mods: vec![ModReference {
                name: "synthetic.jar".into(),
                source: "Modrinth".into(),
                project_id: "synthetic".into(),
                version_id: "fixture".into(),
                sha1: format!("{:x}", sha1::Sha1::digest(b"synthetic")),
                sha512: format!("{:x}", sha2::Sha512::digest(b"synthetic")),
                url: "https://cdn.modrinth.com/data/synthetic/versions/fixture/synthetic.jar"
                    .into(),
                size: 9,
            }],
            configs: choices
                .iter()
                .map(|(id, (name, data))| ConfigChoice {
                    id: id.clone(),
                    name: name.clone(),
                    size: data.len() as u64,
                    text: String::from_utf8(data.clone()).unwrap(),
                })
                .collect(),
            excluded_mods: vec![],
        };
        let target = root.join("share.mrpack");
        write(
            &root,
            &target,
            &preview,
            &choices.keys().cloned().collect::<Vec<_>>(),
        )
        .unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(&target).unwrap()).unwrap();
        assert_eq!(zip.len(), 2);
        assert_eq!(cobble_content::read_mrpack(&target).unwrap().files.len(), 1);
        assert!(zip.by_name("overrides/config/synthetic.cfg").is_ok());
        assert!(zip.by_name("overrides/mods/unknown.jar").is_err());
        assert!(
            write(
                &root,
                &root.join("invalid.mrpack"),
                &preview,
                &["../accounts.json".into()]
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn local_preview_excludes_unknown_jars_and_revision_tracks_changes() {
        let root = root();
        fs::create_dir(root.join("mods")).unwrap();
        fs::write(root.join("mods/unknown.jar"), b"synthetic jar").unwrap();
        fs::write(root.join("config/client.cfg"), b"enabled=true").unwrap();
        let first = preview(&root, "Synthetic", "1.21.1", None, None)
            .await
            .unwrap();
        assert!(first.mods.is_empty());
        assert_eq!(first.excluded_mods, vec!["unknown.jar"]);
        assert_eq!(first.configs.len(), 1);
        let output = root.join("friends.mrpack");
        write(&root, &output, &first, &[]).unwrap();
        let archive = zip::ZipArchive::new(fs::File::open(output).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        fs::write(root.join("config/client.cfg"), b"enabled=false").unwrap();
        let second = preview(&root, "Synthetic", "1.21.1", None, None)
            .await
            .unwrap();
        assert_ne!(first.revision, second.revision);
        assert!(
            write(
                &root,
                &root.join("stale.mrpack"),
                &first,
                &[first.configs[0].id.clone()]
            )
            .is_err()
        );
        assert!(!root.join("stale.mrpack").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn thousand_mod_scan_and_reference_archive_stay_bounded() {
        let root = root();
        fs::create_dir(root.join("mods")).unwrap();
        for n in 0..1024 {
            fs::write(root.join(format!("mods/synthetic-{n:04}.jar")), b"fixture").unwrap();
        }
        let stamps = mod_bytes(&root).unwrap();
        assert_eq!(stamps.len(), 1024);
        let mut plan = preview(&root, "Synthetic large pack", "1.21.1", None, None)
            .await
            .unwrap();
        assert_eq!(plan.excluded_mods.len(), 1024);
        plan.mods = stamps
            .into_iter()
            .enumerate()
            .map(|(n, (name, (sha1, size, sha512)))| ModReference {
                url: format!("https://cdn.modrinth.com/data/synthetic/versions/fixture/{name}"),
                name,
                source: "Modrinth".into(),
                project_id: format!("synthetic{n}"),
                version_id: "fixture".into(),
                sha1,
                sha512,
                size,
            })
            .collect();
        plan.excluded_mods.clear();
        assert!(serde_json::to_vec(&plan).unwrap().len() < 1_048_576);
        let output = root.join("synthetic-1024.mrpack");
        write(&root, &output, &plan, &[]).unwrap();
        assert_eq!(
            cobble_content::read_mrpack(&output).unwrap().files.len(),
            1024
        );
        assert_eq!(
            zip::ZipArchive::new(fs::File::open(&output).unwrap())
                .unwrap()
                .len(),
            1
        );
        fs::write(root.join("mods/one-more.jar"), b"fixture").unwrap();
        assert!(mod_bytes(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

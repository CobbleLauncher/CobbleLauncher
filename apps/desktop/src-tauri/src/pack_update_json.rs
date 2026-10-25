// SPDX-License-Identifier: MIT
//! Scoped JSON bytes retained only in the host's expiring copy plan.
use super::*;
use cobble_content::pack_update_preview::{
    self as domain, JsonConfigPreview, JsonFieldChoice, JsonMergeDocument,
};
use std::{collections::BTreeMap, io::Read, path::Path};

#[derive(Clone)]
pub(crate) struct PreparedJson {
    pub document: JsonMergeDocument,
    pub local_sha256: [u8; 32],
}
fn overrides(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let result = (|| -> Result<_, cobble_content::BoxError> {
        let file = fs::File::open(path)?;
        let pack = cobble_content::read_mrpack_file(file.try_clone()?)?;
        let mut zip = zip::ZipArchive::new(file)?;
        let mut result = BTreeMap::new();
        let mut total = 0;
        for entry in pack.overrides {
            if !(entry.path.starts_with("config/") || entry.path.starts_with("defaultconfigs/"))
                || !entry.path.ends_with(".json")
                || entry.size > domain::MAX_JSON_MERGE_BYTES as u64
            {
                continue;
            }
            let mut file = zip.by_index(entry.entry_index)?;
            let mut bytes = Vec::new();
            Read::by_ref(&mut file)
                .take(domain::MAX_JSON_MERGE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != entry.size || bytes.len() > domain::MAX_JSON_MERGE_BYTES {
                continue;
            }
            total += bytes.len();
            if total > 2 * 1024 * 1024 {
                return Ok(BTreeMap::new());
            }
            result.insert(entry.path, bytes);
        }
        Ok(result)
    })();
    result.unwrap_or_default()
}
pub(crate) fn collect(
    game: &Game,
    base: &Path,
    target: &Path,
    preview: &PackUpdatePreview,
) -> Vec<PreparedJson> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    use cap_std::fs::{Dir, OpenOptions};
    let before = overrides(base);
    let after = overrides(target);
    let Ok(root) = Dir::open_ambient_dir(&game.directory, cap_std::ambient_authority()) else {
        return vec![];
    };
    let mut result = Vec::new();
    let mut budget = 2 * 1024 * 1024;
    for (path, new) in after {
        let Some(old) = before.get(&path) else {
            continue;
        };
        let metadata = preview
            .version_changed
            .iter()
            .chain(&preview.unchanged)
            .find_map(|change| match (&change.before, &change.after) {
                (Some(b), Some(a)) if b.path == path && a.path == path => Some((b, a)),
                _ => None,
            });
        let Some((b, a)) = metadata else {
            continue;
        };
        if b.sha512.as_ref() != Some(&format!("{:x}", sha2::Sha512::digest(old)))
            || a.sha512.as_ref() != Some(&format!("{:x}", sha2::Sha512::digest(&new)))
        {
            continue;
        }
        let Some(observed) =
            super::super::pack_update_preview::observation(&root, &path, &mut budget)
        else {
            continue;
        };
        if !observed.exists || observed.size_bytes > domain::MAX_JSON_MERGE_BYTES as u64 {
            continue;
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let Ok(mut file) = root.open_with(&path, &options) else {
            continue;
        };
        let mut bytes = Vec::new();
        if Read::by_ref(&mut file)
            .take(domain::MAX_JSON_MERGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() as u64 != observed.size_bytes
            || observed.sha512.as_ref() != Some(&format!("{:x}", sha2::Sha512::digest(&bytes)))
        {
            continue;
        }
        if let Some(document) = domain::preview_json_merge(&path, old, &bytes, &new)
            && !document.preview.fields.is_empty()
        {
            result.push(PreparedJson {
                document,
                local_sha256: sha2::Sha256::digest(&bytes).into(),
            });
        }
    }
    // Bound the complete IPC response as well as every individual document.
    let size = result.iter().try_fold(0usize, |sum, item| {
        serde_json::to_vec(&item.document.preview)
            .ok()
            .and_then(|bytes| sum.checked_add(bytes.len()))
    });
    if size.is_none_or(|size| size > 2 * 1024 * 1024) {
        vec![]
    } else {
        result
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MergeRequest {
    pub path: String,
    pub fields: Vec<JsonFieldChoice>,
}
pub(super) fn choices(
    documents: &BTreeMap<String, JsonMergeDocument>,
    requests: &[MergeRequest],
    preserved: &std::collections::BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if requests.len() > documents.len() {
        return Err("PACK_UPDATE_JSON_SELECTION_INVALID".into());
    }
    let mut result = BTreeMap::new();
    for request in requests {
        if preserved.contains(&request.path) || result.contains_key(&request.path) {
            return Err("PACK_UPDATE_JSON_SELECTION_INVALID".into());
        }
        let doc = documents
            .get(&request.path)
            .ok_or("PACK_UPDATE_JSON_SELECTION_INVALID")?;
        result.insert(
            request.path.clone(),
            domain::merge_json_document(doc, &request.fields).map_err(String::from)?,
        );
    }
    Ok(result)
}
pub(super) fn write(
    target: &Path,
    path: &str,
    bytes: &[u8],
    cancel: &AtomicBool,
) -> Result<(), String> {
    use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
    use cap_std::fs::{Dir, OpenOptions};
    if cancel.load(Ordering::Acquire) {
        return Err("PACK_UPDATE_CANCELLED".into());
    }
    cobble_store::SafeRelativePath::parse(path).map_err(|_| code())?;
    let root = Dir::open_ambient_dir(target, cap_std::ambient_authority()).map_err(|_| code())?;
    let mut parent = PathBuf::new();
    for part in Path::new(path).parent().ok_or_else(code)?.components() {
        parent.push(part.as_os_str());
        let meta = root.symlink_metadata(&parent).map_err(|_| code())?;
        if !meta.is_dir() || meta.file_type().is_symlink() || is_cap_reparse_point(&meta) {
            return Err("PACK_UPDATE_LINK_DENIED".into());
        }
    }
    let temp = format!("{path}.cobble-json-merge-tmp");
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    let mut output = root.open_with(&temp, &options).map_err(|_| code())?;
    output.write_all(bytes).map_err(|_| code())?;
    output.sync_all().map_err(|_| code())?;
    drop(output);
    if cancel.load(Ordering::Acquire) {
        let _ = root.remove_file(&temp);
        return Err("PACK_UPDATE_CANCELLED".into());
    }
    remove_new_file(target, path)?;
    root.rename(&temp, &root, path).map_err(|_| code())
}

pub(super) fn previews(documents: &BTreeMap<String, JsonMergeDocument>) -> Vec<JsonConfigPreview> {
    documents.values().map(|d| d.preview.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pack(path: &Path, version: &str, bytes: &[u8]) {
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("modrinth.index.json", options).unwrap();
        zip.write_all(serde_json::to_string(&serde_json::json!({"formatVersion":1,"game":"minecraft","versionId":version,"name":"JSON fixture","files":[],"dependencies":{"minecraft":"1.21.1"}})).unwrap().as_bytes()).unwrap();
        zip.start_file("overrides/config/options.json", options)
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
    }
    #[test]
    fn json_copy_reads_verified_archives_merges_only_copy_and_detects_local_revision() {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original");
        let copy = temp.path().join("copy");
        fs::create_dir_all(original.join("config")).unwrap();
        fs::create_dir_all(copy.join("config")).unwrap();
        let old = temp.path().join("old.mrpack");
        let new = temp.path().join("new.mrpack");
        pack(&old, "old", br#"{"conflict":1,"keep":true}"#);
        pack(&new, "new", br#"{"conflict":3,"keep":true,"upstream":8}"#);
        let local = br#"{"conflict":2,"keep":true,"player":7}"#;
        fs::write(original.join("config/options.json"), local).unwrap();
        fs::write(
            copy.join("config/options.json"),
            br#"{"conflict":3,"keep":true,"upstream":8}"#,
        )
        .unwrap();
        let game:Game=serde_json::from_value(serde_json::json!({"id":"json-fixture","name":"JSON fixture","version":"1.21.1","directory":original,"status":"installed"})).unwrap();
        let before = domain::snapshot_mrpack(&old, &[]).unwrap();
        let after = domain::snapshot_mrpack(&new, &[]).unwrap();
        let observation = domain::ObservedPackFile {
            path: "config/options.json".into(),
            exists: true,
            size_bytes: local.len() as u64,
            sha1: None,
            sha512: Some(format!("{:x}", sha2::Sha512::digest(local))),
        };
        let preview = domain::preview_pack_update(Some(&before), &after, &[observation]).unwrap();
        let prepared = collect(&game, &old, &new, &preview);
        assert_eq!(prepared.len(), 1);
        let documents =
            BTreeMap::from([("config/options.json".into(), prepared[0].document.clone())]);
        let requests = [MergeRequest {
            path: "config/options.json".into(),
            fields: vec![JsonFieldChoice {
                pointer: "/conflict".into(),
                side: domain::JsonMergeSide::Local,
            }],
        }];
        let merged = choices(&documents, &requests, &BTreeSet::new()).unwrap();
        write(
            &copy,
            "config/options.json",
            &merged["config/options.json"],
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            fs::read(original.join("config/options.json")).unwrap(),
            local
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(copy.join("config/options.json")).unwrap()
            )
            .unwrap(),
            serde_json::json!({"conflict":2,"keep":true,"player":7,"upstream":8})
        );
        let (stamps, _) = local_candidates(&game, &preview).unwrap();
        assert_eq!(
            stamps["config/options.json"].sha256,
            prepared[0].local_sha256
        );
        fs::write(original.join("config/options.json"), b"{}").unwrap();
        let (changed, _) = local_candidates(&game, &preview).unwrap();
        assert_ne!(stamps, changed);
        assert_eq!(
            write(&copy, "config/options.json", b"{}", &AtomicBool::new(true)).unwrap_err(),
            "PACK_UPDATE_CANCELLED"
        );
    }
    #[test]
    fn json_copy_missing_or_unreliable_baseline_and_unknown_choices_stay_whole_file() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("new.mrpack");
        pack(&target, "new", b"{}");
        let game:Game=serde_json::from_value(serde_json::json!({"id":"fixture","name":"fixture","version":"1.21.1","directory":temp.path(),"status":"installed"})).unwrap();
        let snapshot = domain::snapshot_mrpack(&target, &[]).unwrap();
        let preview = domain::preview_pack_update(Some(&snapshot), &snapshot, &[]).unwrap();
        assert!(
            collect(
                &game,
                &temp.path().join("missing.mrpack"),
                &target,
                &preview
            )
            .is_empty()
        );
        assert_eq!(
            choices(
                &BTreeMap::new(),
                &[MergeRequest {
                    path: "config/foreign.json".into(),
                    fields: vec![]
                }],
                &BTreeSet::new()
            )
            .unwrap_err(),
            "PACK_UPDATE_JSON_SELECTION_INVALID"
        );
    }
}

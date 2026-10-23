// SPDX-License-Identifier: MIT
//! Pure whole-pack comparison plus a bounded read-only archive snapshot helper.
//! No downloads, extraction, instance mutation or apply plan.
//! Unchanged means declared metadata matches, not that installed bytes were verified.
mod models;
mod snapshot;
use cobble_store::SafeRelativePath;
pub use models::*;
pub use snapshot::{snapshot_mrpack, snapshot_mrpack_file};
use std::collections::{BTreeMap, BTreeSet};
fn digest(value: &Option<String>, length: usize) -> bool {
    value
        .as_ref()
        .is_none_or(|v| v.len() == length && v.bytes().all(|c| c.is_ascii_hexdigit()))
}
fn metadata(path: &str, size: u64, sha1: &Option<String>, sha512: &Option<String>) -> bool {
    SafeRelativePath::parse(path).is_ok()
        && path.len() <= 1024
        && size <= 2 * 1024 * 1024 * 1024
        && digest(sha1, 40)
        && digest(sha512, 128)
}
fn validate(pack: &PackManifestSnapshot) -> Result<(), PackUpdatePreviewError> {
    let mut paths = crate::mrpack::Namespace::default();
    let mut ids = BTreeSet::new();
    if pack.files.len() > 50_000
        || pack.version_id.is_empty()
        || pack.version_id.len() > 512
        || pack.minecraft_version.is_empty()
    {
        return Err(PackUpdatePreviewError::InvalidManifest);
    }
    for file in &pack.files {
        if file.identity.is_empty()
            || file.identity.len() > 1024
            || !ids.insert(&file.identity)
            || !metadata(&file.path, file.size_bytes, &file.sha1, &file.sha512)
            || paths.insert(&file.path, false, Some(0)).is_err()
        {
            return Err(PackUpdatePreviewError::InvalidManifest);
        }
    }
    Ok(())
}
fn protected(path: &str) -> bool {
    let first = path.split('/').next().unwrap_or_default();
    first.eq_ignore_ascii_case("config")
        || first.eq_ignore_ascii_case("saves")
        || first.starts_with(".cobble")
}
fn same_hash(a: &Option<String>, b: &Option<String>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        (None, None) => true,
        _ => false,
    }
}
fn same_file(a: &PackManifestFile, b: &PackManifestFile) -> bool {
    a.path == b.path
        && a.version_id == b.version_id
        && a.size_bytes == b.size_bytes
        && same_hash(&a.sha1, &b.sha1)
        && same_hash(&a.sha512, &b.sha512)
}
fn local(
    before: Option<&PackManifestFile>,
    observation: Option<&&ObservedPackFile>,
) -> PackLocalState {
    let Some(o) = observation else {
        return PackLocalState::Unknown;
    };
    if !o.exists {
        return PackLocalState::Missing;
    }
    let Some(b) = before else {
        return PackLocalState::Unknown;
    };
    if b.size_bytes != o.size_bytes {
        return PackLocalState::Modified;
    }
    let mut compared = false;
    for (a, z) in [(&b.sha1, &o.sha1), (&b.sha512, &o.sha512)] {
        if let (Some(a), Some(z)) = (a, z) {
            compared = true;
            if !a.eq_ignore_ascii_case(z) {
                return PackLocalState::Modified;
            }
        }
    }
    if compared {
        PackLocalState::Clean
    } else {
        PackLocalState::Unknown
    }
}
fn change(
    before: Option<&PackManifestFile>,
    after: Option<&PackManifestFile>,
    observed: &BTreeMap<&str, &ObservedPackFile>,
) -> PackFileChange {
    let path = before.or(after).expect("one side exists").path.as_str();
    let mut local_state = local(before, observed.get(path));
    // A renamed replacement can also collide with a separate user-created target.
    if let (Some(b), Some(a)) = (before, after)
        && b.path != a.path
        && !observed.get(a.path.as_str()).is_some_and(|o| !o.exists)
    {
        local_state = PackLocalState::Unknown;
    }
    let preserve = protected(path)
        || after.is_some_and(|f| protected(&f.path))
        || matches!(
            local_state,
            PackLocalState::Modified | PackLocalState::Unknown
        );
    PackFileChange {
        before: before.cloned(),
        after: after.cloned(),
        local_state,
        preserve,
    }
}
pub fn preview_pack_update(
    installed: Option<&PackManifestSnapshot>,
    target: &PackManifestSnapshot,
    observed: &[ObservedPackFile],
) -> Result<PackUpdatePreview, PackUpdatePreviewError> {
    let old = installed.ok_or(PackUpdatePreviewError::MissingProvenance)?;
    validate(old)?;
    validate(target)?;
    if old.source.provider != target.source.provider
        || matches!((&old.source.project_id,&target.source.project_id),(Some(a),Some(b)) if a!=b)
    {
        return Err(PackUpdatePreviewError::SourceMismatch);
    }
    let mut observations = BTreeMap::new();
    let mut paths = crate::mrpack::Namespace::default();
    if observed.len() > 100_000 {
        return Err(PackUpdatePreviewError::InvalidObservation);
    }
    for o in observed {
        if !metadata(&o.path, o.size_bytes, &o.sha1, &o.sha512)
            || paths.insert(&o.path, false, Some(0)).is_err()
            || observations.insert(o.path.as_str(), o).is_some()
        {
            return Err(PackUpdatePreviewError::InvalidObservation);
        }
    }
    let old_files: BTreeMap<_, _> = old.files.iter().map(|f| (f.identity.as_str(), f)).collect();
    let new_files: BTreeMap<_, _> = target
        .files
        .iter()
        .map(|f| (f.identity.as_str(), f))
        .collect();
    let mut result = PackUpdatePreview {
        identity_relation: if old.source.project_id.is_some() && target.source.project_id.is_some()
        {
            PackIdentityRelation::Confirmed
        } else {
            PackIdentityRelation::Unknown
        },
        current_version: old.version_id.clone(),
        target_version: target.version_id.clone(),
        runtime_changed: old.minecraft_version != target.minecraft_version
            || old.loader_kind != target.loader_kind
            || old.loader_version != target.loader_version,
        added: vec![],
        removed: vec![],
        version_changed: vec![],
        unchanged: vec![],
    };
    for (id, before) in &old_files {
        match new_files.get(id) {
            None => result
                .removed
                .push(change(Some(before), None, &observations)),
            Some(after) if same_file(before, after) => {
                result
                    .unchanged
                    .push(change(Some(before), Some(after), &observations))
            }
            Some(after) => {
                result
                    .version_changed
                    .push(change(Some(before), Some(after), &observations))
            }
        }
    }
    for (id, after) in new_files {
        if !old_files.contains_key(id) {
            result.added.push(change(None, Some(after), &observations));
        }
    }
    for rows in [
        &mut result.added,
        &mut result.removed,
        &mut result.version_changed,
        &mut result.unchanged,
    ] {
        rows.sort_by(|a, b| {
            a.after
                .as_ref()
                .or(a.before.as_ref())
                .unwrap()
                .path
                .cmp(&b.after.as_ref().or(b.before.as_ref()).unwrap().path)
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests;

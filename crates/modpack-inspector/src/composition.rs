// SPDX-License-Identifier: MIT
//! Pure analysis of host-approved archive metadata. No downloaded bytes are verified.
use crate::{Environment, InspectError, Inspection, MAX_FILES, MAX_INDEX_BYTES, inspect_index};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct InspectionRequest {
    pub current: PackSnapshot,
    #[serde(default)]
    pub baseline: Option<PackSnapshot>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackSnapshot {
    pub index: Value,
    #[serde(default)]
    pub overrides: Vec<OverrideFile>,
}

/// CRC and size are ZIP directory metadata, not a cryptographic content identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideFile {
    pub path: String,
    pub size: u64,
    pub crc32: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all(deserialize = "camelCase"))]
pub struct DeclaredFile {
    pub path: String,
    pub file_size: u64,
    pub hashes: BTreeMap<String, String>,
    pub downloads: Vec<String>,
    pub env: Option<Environment>,
}

#[derive(Debug, Serialize)]
pub struct Composition {
    pub summary: Inspection,
    pub raw_index: Value,
    pub files: Vec<DeclaredFile>,
    pub overrides: Vec<OverrideFile>,
}

#[derive(Debug, Serialize)]
pub struct InspectionReport {
    pub current: Composition,
    pub baseline: Option<Composition>,
    pub diff: Option<PackDiff>,
}

#[derive(Debug, Serialize)]
pub struct PackDiff {
    pub comparison_basis: &'static str,
    pub declared_files: FileDiff<DeclaredFile>,
    pub overrides: FileDiff<OverrideFile>,
}

#[derive(Debug, Serialize)]
pub struct FileDiff<T> {
    pub added: Vec<T>,
    pub removed: Vec<T>,
    pub changed: Vec<FileChange<T>>,
    pub unchanged: Vec<T>,
}

#[derive(Debug, Serialize)]
pub struct FileChange<T> {
    pub path: String,
    pub before: T,
    pub after: T,
}

/// Summarizes the complete snapshot and optionally compares against the baseline.
/// Success says nothing about installability, compatibility or downloaded content.
pub fn inspect_request(request: &InspectionRequest) -> Result<InspectionReport, InspectError> {
    let current = compose(&request.current)?;
    let baseline = request.baseline.as_ref().map(compose).transpose()?;
    let diff = baseline.as_ref().map(|before| PackDiff {
        comparison_basis: "Declared index metadata; override ZIP CRC32 and size metadata. Downloaded bytes are not verified.",
        declared_files: compare(&before.files, &current.files, |f| &f.path),
        overrides: compare(&before.overrides, &current.overrides, |f| &f.path),
    });
    Ok(InspectionReport {
        current,
        baseline,
        diff,
    })
}

pub(crate) fn compose(snapshot: &PackSnapshot) -> Result<Composition, InspectError> {
    let bytes = serde_json::to_vec(&snapshot.index).map_err(|_| InspectError::InvalidJson)?;
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(InspectError::InputTooLarge);
    }
    let summary = inspect_index(&bytes)?;
    let files: Vec<DeclaredFile> = serde_json::from_value(snapshot.index["files"].clone())
        .map_err(|_| InspectError::InvalidMetadata)?;
    if snapshot.overrides.len() > MAX_FILES {
        return Err(InspectError::TooManyFiles);
    }
    unique_paths(files.iter().map(|file| file.path.as_str()))?;
    unique_paths(snapshot.overrides.iter().map(|file| file.path.as_str()))?;
    let mut overrides = snapshot.overrides.clone();
    overrides.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Composition {
        summary,
        raw_index: snapshot.index.clone(),
        files,
        overrides,
    })
}

fn unique_paths<'a>(paths: impl Iterator<Item = &'a str>) -> Result<(), InspectError> {
    let mut seen = BTreeSet::new();
    for path in paths {
        if path.is_empty() || !seen.insert(path.to_lowercase()) {
            return Err(InspectError::InvalidInventory);
        }
    }
    Ok(())
}

fn compare<T: Clone + PartialEq>(
    before: &[T],
    after: &[T],
    path: impl Fn(&T) -> &str,
) -> FileDiff<T> {
    let old: BTreeMap<_, _> = before.iter().map(|file| (path(file), file)).collect();
    let new: BTreeMap<_, _> = after.iter().map(|file| (path(file), file)).collect();
    let mut result = FileDiff {
        added: vec![],
        removed: vec![],
        changed: vec![],
        unchanged: vec![],
    };
    for (name, file) in &new {
        match old.get(name) {
            None => result.added.push((*file).clone()),
            Some(previous) if *previous == *file => result.unchanged.push((*file).clone()),
            Some(previous) => result.changed.push(FileChange {
                path: (*name).to_owned(),
                before: (*previous).clone(),
                after: (*file).clone(),
            }),
        }
    }
    for (name, file) in old {
        if !new.contains_key(name) {
            result.removed.push(file.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot() -> PackSnapshot {
        // Original synthetic fixture, no downloaded metadata or third-party files.
        PackSnapshot {
            index: json!({"formatVersion":1,"game":"minecraft","name":"Synthetic",
            "versionId":"one","summary":"Retained raw field","dependencies":{"minecraft":"1.21.1","unknown-loader":"x"},
            "files":[{"path":"mods/a.jar","fileSize":12,"hashes":{"sha1":"a","sha512":"b"},"downloads":["https://example.invalid/a"]},
                     {"path":"mods/old.jar","fileSize":2,"hashes":{"sha512":"c"},"downloads":[]}]}),
            overrides: vec![OverrideFile {
                path: "overrides/config/a.txt".into(),
                size: 3,
                crc32: 42,
            }],
        }
    }

    #[test]
    fn preserves_composition_and_classifies_file_and_override_deltas() {
        let before = snapshot();
        let mut current = snapshot();
        current.index["files"][0]["hashes"]["sha512"] = json!("changed");
        current.index["files"][1]["path"] = json!("mods/new.jar");
        current.overrides[0].crc32 = 43;
        let report = inspect_request(&InspectionRequest {
            current,
            baseline: Some(before),
        })
        .unwrap();
        assert_eq!(report.current.raw_index["summary"], "Retained raw field");
        assert_eq!(report.current.summary.dependencies["unknown-loader"], "x");
        let diff = report.diff.unwrap();
        assert_eq!(diff.declared_files.added.len(), 1);
        assert_eq!(diff.declared_files.removed.len(), 1);
        assert_eq!(diff.declared_files.changed.len(), 1);
        assert_eq!(diff.overrides.changed.len(), 1);
        assert!(diff.comparison_basis.contains("not verified"));
    }

    #[test]
    fn identical_inventory_is_unchanged_and_duplicate_paths_rejected() {
        let report = inspect_request(&InspectionRequest {
            current: snapshot(),
            baseline: Some(snapshot()),
        })
        .unwrap();
        let diff = report.diff.unwrap();
        assert_eq!(diff.declared_files.unchanged.len(), 2);
        assert_eq!(diff.overrides.unchanged.len(), 1);
        let mut current = snapshot();
        current.index["files"][1]["path"] = json!("MODS/A.JAR");
        assert!(matches!(
            inspect_request(&InspectionRequest {
                current,
                baseline: None
            }),
            Err(InspectError::InvalidInventory)
        ));
    }
}

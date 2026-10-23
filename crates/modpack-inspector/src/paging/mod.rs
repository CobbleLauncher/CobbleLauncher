// SPDX-License-Identifier: MIT
//! Immutable inventories with index-only delta rows and bounded query results.
mod query;
mod types;
pub use types::*;
#[cfg(test)]
mod tests;

use crate::{Composition, InspectError, InspectionRequest, composition::compose};
use std::{borrow::Cow, collections::BTreeMap, fmt, sync::Mutex};

#[derive(Debug, Clone, Copy)]
enum Pack {
    Current,
    Baseline,
}
impl Pack {
    fn label(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Baseline => "baseline",
        }
    }
}
#[derive(Debug, Clone, Copy)]
struct FileRef {
    pack: Pack,
    source: RowSource,
    index: usize,
}
#[derive(Debug)]
enum RowPath {
    File(FileRef),
    Metadata {
        pack: Pack,
        key: String,
        dependency: bool,
    },
    Label(String),
}
#[derive(Debug, Clone, Copy)]
enum ValueRef {
    File(FileRef),
    Metadata,
    Count(u64),
    Basis,
}
#[derive(Debug)]
struct Row {
    source: RowSource,
    change: Option<ChangeKind>,
    path: RowPath,
    before: Option<ValueRef>,
    after: Option<ValueRef>,
}
struct DetailCache {
    row: usize,
    field: DetailField,
    text: String,
}

/// A prepared native/SDK session. Files remain in their original composition;
/// comparison rows hold only indices into those files, never cloned deltas.
pub struct PreparedInspection {
    current: Composition,
    baseline: Option<Composition>,
    rows: Vec<Row>,
    overview: Vec<usize>,
    current_rows: Vec<usize>,
    baseline_rows: Vec<usize>,
    changes: Vec<usize>,
    counts: Option<ChangeCounts>,
    // Only the most recently requested detail field is cached, bounded by source
    // metadata limits. Paging a long field must not serialize it on every chunk.
    detail_cache: Mutex<Option<DetailCache>>,
}
impl fmt::Debug for PreparedInspection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedInspection")
            .field("rows", &self.rows.len())
            .finish()
    }
}

impl PreparedInspection {
    pub fn new(request: InspectionRequest) -> Result<Self, InspectError> {
        let current = compose(&request.current)?;
        let baseline = request.baseline.as_ref().map(compose).transpose()?;
        let mut prepared = Self {
            current,
            baseline,
            rows: vec![],
            overview: vec![],
            current_rows: vec![],
            baseline_rows: vec![],
            changes: vec![],
            counts: None,
            detail_cache: Mutex::new(None),
        };
        for pack in [Pack::Current, Pack::Baseline] {
            if matches!(pack, Pack::Baseline) && prepared.baseline.is_none() {
                continue;
            }
            let (metadata, dependencies, file_count, override_count, download_bytes) = {
                let composition = prepared.pack(pack);
                (
                    composition
                        .raw_index
                        .as_object()
                        .expect("validated index")
                        .keys()
                        .filter(|key| !matches!(key.as_str(), "files" | "dependencies"))
                        .cloned()
                        .collect::<Vec<_>>(),
                    composition
                        .summary
                        .dependencies
                        .keys()
                        .cloned()
                        .collect::<Vec<_>>(),
                    composition.files.len(),
                    composition.overrides.len(),
                    composition.summary.declared_download_bytes,
                )
            };
            for (dependency, keys) in [(false, metadata), (true, dependencies)] {
                for key in keys {
                    let value = Some(ValueRef::Metadata);
                    let index = prepared.push(Row {
                        source: if dependency {
                            RowSource::Dependency
                        } else {
                            RowSource::Metadata
                        },
                        change: None,
                        path: RowPath::Metadata {
                            pack,
                            key,
                            dependency,
                        },
                        before: if matches!(pack, Pack::Baseline) {
                            value
                        } else {
                            None
                        },
                        after: if matches!(pack, Pack::Current) {
                            value
                        } else {
                            None
                        },
                    });
                    prepared.overview.push(index);
                }
            }
            for (name, count) in [
                ("declared_files", file_count as u64),
                ("override_files", override_count as u64),
                ("declared_download_bytes", download_bytes),
            ] {
                let value = Some(ValueRef::Count(count));
                let index = prepared.push(Row {
                    source: RowSource::Metadata,
                    change: None,
                    path: RowPath::Label(format!("{}/{name}", pack.label())),
                    before: if matches!(pack, Pack::Baseline) {
                        value
                    } else {
                        None
                    },
                    after: if matches!(pack, Pack::Current) {
                        value
                    } else {
                        None
                    },
                });
                prepared.overview.push(index);
            }
            for (source, count) in [
                (RowSource::DeclaredFile, file_count),
                (RowSource::OverrideFile, override_count),
            ] {
                for index in 0..count {
                    let file = FileRef {
                        pack,
                        source,
                        index,
                    };
                    let value = Some(ValueRef::File(file));
                    let id = prepared.push(Row {
                        source,
                        change: None,
                        path: RowPath::File(file),
                        before: if matches!(pack, Pack::Baseline) {
                            value
                        } else {
                            None
                        },
                        after: if matches!(pack, Pack::Current) {
                            value
                        } else {
                            None
                        },
                    });
                    match pack {
                        Pack::Current => prepared.current_rows.push(id),
                        Pack::Baseline => prepared.baseline_rows.push(id),
                    }
                }
            }
        }
        if prepared.baseline.is_some() {
            let mut counts = ChangeCounts::default();
            for source in [RowSource::DeclaredFile, RowSource::OverrideFile] {
                let before = prepared.file_map(Pack::Baseline, source);
                let after = prepared.file_map(Pack::Current, source);
                let paths: std::collections::BTreeSet<_> =
                    before.keys().chain(after.keys()).cloned().collect();
                let mut source_counts = ChangeCounts::default();
                for path in paths {
                    let old = before.get(&path).copied();
                    let new = after.get(&path).copied();
                    let change = match (old, new) {
                        (None, Some(_)) => ChangeKind::Added,
                        (Some(_), None) => ChangeKind::Removed,
                        (Some(old), Some(new)) => {
                            if prepared.equal(old, new) {
                                ChangeKind::Unchanged
                            } else {
                                ChangeKind::Changed
                            }
                        }
                        (None, None) => unreachable!("union of file maps"),
                    };
                    counts.add(change);
                    source_counts.add(change);
                    let id = prepared.push(Row {
                        source,
                        change: Some(change),
                        path: RowPath::File(new.or(old).expect("file exists")),
                        before: old.map(ValueRef::File),
                        after: new.map(ValueRef::File),
                    });
                    prepared.changes.push(id);
                }
                for (kind, count) in [
                    (ChangeKind::Added, source_counts.added),
                    (ChangeKind::Removed, source_counts.removed),
                    (ChangeKind::Changed, source_counts.changed),
                    (ChangeKind::Unchanged, source_counts.unchanged),
                ] {
                    let id = prepared.push(Row {
                        source: RowSource::Comparison,
                        change: Some(kind),
                        path: RowPath::Label(format!(
                            "comparison/{} {}",
                            source.label(),
                            kind.label()
                        )),
                        before: None,
                        after: Some(ValueRef::Count(u64::from(count))),
                    });
                    prepared.overview.push(id);
                }
            }
            prepared.counts = Some(counts);
            let id = prepared.push(Row {
                source: RowSource::Comparison,
                change: None,
                path: RowPath::Label("comparison/basis".into()),
                before: None,
                after: Some(ValueRef::Basis),
            });
            prepared.overview.push(id);
        }
        for section in [
            InspectionSection::Overview,
            InspectionSection::Current,
            InspectionSection::Baseline,
            InspectionSection::Changes,
        ] {
            let mut sorted = prepared.section(section).to_vec();
            sorted.sort_by(|a, b| {
                prepared
                    .path(&prepared.rows[*a])
                    .cmp(&prepared.path(&prepared.rows[*b]))
                    .then_with(|| prepared.rows[*a].source.cmp(&prepared.rows[*b].source))
                    .then_with(|| a.cmp(b))
            });
            *prepared.section_mut(section) = sorted;
        }
        Ok(prepared)
    }

    pub fn summary(&self) -> InspectionSummary {
        let summary = |composition: &Composition| PackSummary {
            name: preview(&composition.summary.name),
            version_id: preview(&composition.summary.version_id),
            declared_files: composition.files.len() as u32,
            override_files: composition.overrides.len() as u32,
            declared_download_bytes: composition.summary.declared_download_bytes,
        };
        InspectionSummary {
            current: summary(&self.current),
            baseline: self.baseline.as_ref().map(summary),
            changes: self.counts.clone(),
        }
    }

    fn push(&mut self, row: Row) -> usize {
        let index = self.rows.len();
        self.rows.push(row);
        index
    }
    fn pack(&self, pack: Pack) -> &Composition {
        match pack {
            Pack::Current => &self.current,
            Pack::Baseline => self.baseline.as_ref().expect("baseline reference"),
        }
    }
    fn file_map(&self, pack: Pack, source: RowSource) -> BTreeMap<String, FileRef> {
        let composition = self.pack(pack);
        match source {
            RowSource::DeclaredFile => composition
                .files
                .iter()
                .enumerate()
                .map(|(index, file)| {
                    (
                        file.path.clone(),
                        FileRef {
                            pack,
                            source,
                            index,
                        },
                    )
                })
                .collect(),
            RowSource::OverrideFile => composition
                .overrides
                .iter()
                .enumerate()
                .map(|(index, file)| {
                    (
                        file.path.clone(),
                        FileRef {
                            pack,
                            source,
                            index,
                        },
                    )
                })
                .collect(),
            _ => unreachable!("file source"),
        }
    }
    fn equal(&self, before: FileRef, after: FileRef) -> bool {
        match before.source {
            RowSource::DeclaredFile => {
                self.pack(before.pack).files[before.index]
                    == self.pack(after.pack).files[after.index]
            }
            RowSource::OverrideFile => {
                self.pack(before.pack).overrides[before.index]
                    == self.pack(after.pack).overrides[after.index]
            }
            _ => unreachable!("file source"),
        }
    }
    fn path<'a>(&'a self, row: &'a Row) -> Cow<'a, str> {
        match &row.path {
            RowPath::Label(label) => Cow::Borrowed(label),
            RowPath::Metadata {
                pack,
                key,
                dependency,
            } => Cow::Owned(format!(
                "{}/{}{key}",
                pack.label(),
                if *dependency { "dependencies/" } else { "" }
            )),
            RowPath::File(file) => Cow::Borrowed(match file.source {
                RowSource::DeclaredFile => &self.pack(file.pack).files[file.index].path,
                RowSource::OverrideFile => &self.pack(file.pack).overrides[file.index].path,
                _ => unreachable!("file source"),
            }),
        }
    }
    fn value(&self, row: &Row, value: ValueRef) -> String {
        match value {
        ValueRef::Count(count)=>count.to_string(),
        ValueRef::Basis=>"Declared index metadata; override ZIP CRC32 and size metadata. Downloaded bytes are not verified.".into(),
        ValueRef::Metadata=>match &row.path {RowPath::Metadata {pack,key,dependency}=>{let raw=&self.pack(*pack).raw_index;if *dependency {raw["dependencies"][key].to_string()}else{raw[key].to_string()}},_=>unreachable!("metadata reference")},
        ValueRef::File(file)=>match file.source {
            RowSource::DeclaredFile=>self.pack(file.pack).raw_index["files"][file.index].to_string(),
            RowSource::OverrideFile=>serde_json::to_string(&self.pack(file.pack).overrides[file.index]).expect("serializable metadata"),
            _=>unreachable!("file source"),
        },
    }
    }
    fn section(&self, section: InspectionSection) -> &[usize] {
        match section {
            InspectionSection::Overview => &self.overview,
            InspectionSection::Current => &self.current_rows,
            InspectionSection::Baseline => &self.baseline_rows,
            InspectionSection::Changes => &self.changes,
        }
    }
    fn section_mut(&mut self, section: InspectionSection) -> &mut Vec<usize> {
        match section {
            InspectionSection::Overview => &mut self.overview,
            InspectionSection::Current => &mut self.current_rows,
            InspectionSection::Baseline => &mut self.baseline_rows,
            InspectionSection::Changes => &mut self.changes,
        }
    }
}

fn preview(text: &str) -> CellPreview {
    let mut end = text.len().min(MAX_PREVIEW_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    CellPreview {
        text: text[..end].to_owned(),
        byte_length: text.len() as u32,
        complete: end == text.len(),
    }
}

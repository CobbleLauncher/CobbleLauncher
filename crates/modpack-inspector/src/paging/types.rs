// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};
use std::fmt;

pub const INSPECTION_PAGE_PROTOCOL: &str = "cobble.mrpack-page@1";
pub const MAX_PAGE_ROWS: u32 = 100;
pub const MAX_PREVIEW_BYTES: usize = 512;
pub const MAX_DETAIL_BYTES: u32 = 2048;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionSection {
    #[default]
    Overview,
    Current,
    Baseline,
    Changes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InspectionQuery {
    pub section: InspectionSection,
    pub search: String,
    pub offset: u32,
    pub limit: u32,
    #[serde(default = "show_dependencies_by_default")]
    pub show_dependencies: bool,
}
fn show_dependencies_by_default() -> bool {
    true
}
impl Default for InspectionQuery {
    fn default() -> Self {
        Self {
            section: InspectionSection::Overview,
            search: String::new(),
            offset: 0,
            limit: 50,
            show_dependencies: true,
        }
    }
}
impl InspectionQuery {
    pub fn validate(&self) -> Result<(), QueryError> {
        if !(1..=MAX_PAGE_ROWS).contains(&self.limit)
            || self.search.len() > 1024
            || self.search.chars().count() > 256
            || self.search.chars().any(char::is_control)
        {
            return Err(QueryError::InvalidQuery);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowSource {
    Metadata,
    Dependency,
    DeclaredFile,
    OverrideFile,
    Comparison,
}
impl RowSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Metadata => "metadata",
            Self::Dependency => "dependency",
            Self::DeclaredFile => "declared_file",
            Self::OverrideFile => "override_file",
            Self::Comparison => "comparison",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Unchanged,
}
impl ChangeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
            Self::Unchanged => "unchanged",
        }
    }
}

/// A labelled prefix, never a replacement for the exact field. Retrieve the field
/// through the detail query when complete is false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellPreview {
    pub text: String,
    pub byte_length: u32,
    pub complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionRow {
    pub id: String,
    pub source: RowSource,
    pub change: Option<ChangeKind>,
    pub path: CellPreview,
    pub before: Option<CellPreview>,
    pub after: Option<CellPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionPage {
    pub query: InspectionQuery,
    pub total_rows: u32,
    pub matched_rows: u32,
    pub next_offset: Option<u32>,
    pub rows: Vec<InspectionRow>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailField {
    Path,
    Before,
    #[default]
    After,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InspectionDetailQuery {
    pub row_id: String,
    pub field: DetailField,
    pub offset: u32,
    pub limit: u32,
}
impl Default for InspectionDetailQuery {
    fn default() -> Self {
        Self {
            row_id: String::new(),
            field: DetailField::After,
            offset: 0,
            limit: 1024,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionDetail {
    pub row_id: String,
    pub field: DetailField,
    pub total_bytes: u32,
    pub offset: u32,
    pub next_offset: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackSummary {
    pub name: CellPreview,
    pub version_id: CellPreview,
    pub declared_files: u32,
    pub override_files: u32,
    pub declared_download_bytes: u64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeCounts {
    pub added: u32,
    pub removed: u32,
    pub changed: u32,
    pub unchanged: u32,
}
impl ChangeCounts {
    pub(crate) fn add(&mut self, kind: ChangeKind) {
        match kind {
            ChangeKind::Added => self.added += 1,
            ChangeKind::Removed => self.removed += 1,
            ChangeKind::Changed => self.changed += 1,
            ChangeKind::Unchanged => self.unchanged += 1,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectionSummary {
    pub current: PackSummary,
    pub baseline: Option<PackSummary>,
    pub changes: Option<ChangeCounts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InspectionContent {
    Page { page: InspectionPage },
    Detail { detail: InspectionDetail },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionEnvelope {
    pub protocol: String,
    #[serde(flatten)]
    pub content: InspectionContent,
}
impl InspectionEnvelope {
    pub fn page(page: InspectionPage) -> Self {
        Self {
            protocol: INSPECTION_PAGE_PROTOCOL.into(),
            content: InspectionContent::Page { page },
        }
    }
    pub fn detail(detail: InspectionDetail) -> Self {
        Self {
            protocol: INSPECTION_PAGE_PROTOCOL.into(),
            content: InspectionContent::Detail { detail },
        }
    }
    pub fn validate_protocol(&self) -> Result<(), QueryError> {
        if self.protocol != INSPECTION_PAGE_PROTOCOL {
            return Err(QueryError::InvalidQuery);
        }
        match &self.content {
            InspectionContent::Page { page } => {
                page.query.validate()?;
                if page.matched_rows > page.total_rows || page.query.offset > page.matched_rows {
                    return Err(QueryError::InvalidQuery);
                }
                let count = page.query.limit.min(page.matched_rows - page.query.offset);
                let end = page.query.offset + count;
                let next = (end < page.matched_rows).then_some(end);
                if page.rows.len() != count as usize || page.next_offset != next {
                    return Err(QueryError::InvalidQuery);
                }
                let mut ids = std::collections::BTreeSet::new();
                for row in &page.rows {
                    if !valid_row_id(&row.id) || !ids.insert(&row.id) {
                        return Err(QueryError::InvalidQuery);
                    }
                    for cell in std::iter::once(&row.path)
                        .chain(row.before.iter())
                        .chain(row.after.iter())
                    {
                        if cell.text.len() > MAX_PREVIEW_BYTES
                            || cell.byte_length < (cell.text.len() as u32)
                            || cell.complete != (cell.byte_length == cell.text.len() as u32)
                        {
                            return Err(QueryError::InvalidQuery);
                        }
                    }
                }
            }
            InspectionContent::Detail { detail } => {
                let end = detail
                    .offset
                    .checked_add(detail.text.len() as u32)
                    .ok_or(QueryError::InvalidQuery)?;
                if !valid_row_id(&detail.row_id)
                    || detail.text.len() > MAX_DETAIL_BYTES as usize
                    || end > detail.total_bytes
                    || detail.next_offset != (end < detail.total_bytes).then_some(end)
                    || (detail.text.is_empty() && detail.next_offset.is_some())
                {
                    return Err(QueryError::InvalidQuery);
                }
            }
        }
        Ok(())
    }
}

fn valid_row_id(id: &str) -> bool {
    id.len() <= 32
        && id.strip_prefix('r').is_some_and(|value| {
            !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryError {
    InvalidQuery,
    InvalidRange,
    UnknownRow,
    UnavailableField,
}
impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidQuery => "Invalid inspection query",
            Self::InvalidRange => "Invalid inspection range",
            Self::UnknownRow => "Unknown inspection row",
            Self::UnavailableField => "Inspection field is absent",
        })
    }
}
impl std::error::Error for QueryError {}

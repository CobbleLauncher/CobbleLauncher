// SPDX-License-Identifier: MIT
//! Read-only summaries of caller-provided Modrinth v1 index bytes.
//! This is a domain analyzer, not an installed Wasm plugin or an import validator.
//! It does not open archives, authorize paths, verify hashes, fetch downloads,
//! read override bytes, or determine whether the declared dependencies work together.

mod composition;
pub use composition::*;
mod paging;
pub use paging::*;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_INDEX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_FILES: usize = 50_000;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Inspection {
    pub name: String,
    pub version_id: String,
    /// All declared dependencies, including unknown loader names. No compatibility inference.
    pub dependencies: BTreeMap<String, String>,
    pub file_count: usize,
    /// Sum over every declared file, including files unsupported on either side.
    pub declared_download_bytes: u64,
    pub client: SideCounts,
    pub server: SideCounts,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct SideCounts {
    pub required: usize,
    pub optional: usize,
    pub unsupported: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InspectError {
    InputTooLarge,
    InvalidJson,
    UnsupportedFormat,
    TooManyFiles,
    InvalidMetadata,
    DownloadSizeOverflow,
    InvalidInventory,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    format_version: u32,
    game: String,
    name: String,
    version_id: String,
    dependencies: BTreeMap<String, String>,
    files: Vec<IndexFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IndexFile {
    file_size: u64,
    env: Option<Environment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub client: Side,
    pub server: Side,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Required,
    Optional,
    Unsupported,
}

impl SideCounts {
    fn add(&mut self, side: &Side) {
        match side {
            Side::Required => self.required += 1,
            Side::Optional => self.optional += 1,
            Side::Unsupported => self.unsupported += 1,
        }
    }
}

/// Accepts at most 4 MiB. Omitted environments count as required on both sides.
/// Only summary fields are parsed; success is not proof of a valid installable pack.
pub fn inspect_index(bytes: &[u8]) -> Result<Inspection, InspectError> {
    if bytes.len() > MAX_INDEX_BYTES {
        return Err(InspectError::InputTooLarge);
    }
    let index: Index = serde_json::from_slice(bytes).map_err(|_| InspectError::InvalidJson)?;
    if index.format_version != 1 || index.game != "minecraft" {
        return Err(InspectError::UnsupportedFormat);
    }
    if index.files.len() > MAX_FILES {
        return Err(InspectError::TooManyFiles);
    }
    if index.name.trim().is_empty()
        || index.version_id.trim().is_empty()
        || !index.dependencies.contains_key("minecraft")
        || index
            .dependencies
            .iter()
            .any(|(key, value)| key.trim().is_empty() || value.trim().is_empty())
    {
        return Err(InspectError::InvalidMetadata);
    }
    let mut result = Inspection {
        name: index.name,
        version_id: index.version_id,
        dependencies: index.dependencies,
        file_count: index.files.len(),
        declared_download_bytes: 0,
        client: SideCounts::default(),
        server: SideCounts::default(),
    };
    for file in index.files {
        result.declared_download_bytes = result
            .declared_download_bytes
            .checked_add(file.file_size)
            .ok_or(InspectError::DownloadSizeOverflow)?;
        match file.env {
            Some(env) => {
                result.client.add(&env.client);
                result.server.add(&env.server);
            }
            None => {
                result.client.add(&Side::Required);
                result.server.add(&Side::Required);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> serde_json::Value {
        // Original metadata fixture; no downloaded pack or third-party source.
        json!({"formatVersion":1,"game":"minecraft","name":"Local sample",
            "versionId":"sample-1","dependencies":{"minecraft":"1.21.1",
            "fabric-loader":"0.16.0","future-loader":"unknown"},"files":[
            {"fileSize":10},
            {"fileSize":20,"env":{"client":"optional","server":"unsupported"}},
            {"fileSize":30,"env":{"client":"unsupported","server":"optional"}}]})
    }

    fn inspect(value: &serde_json::Value) -> Result<Inspection, InspectError> {
        inspect_index(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn reports_declared_counts_and_preserves_unknown_dependencies() {
        let report = inspect(&fixture()).unwrap();
        assert_eq!(report.file_count, 3);
        assert_eq!(report.declared_download_bytes, 60);
        assert_eq!(
            report.client,
            SideCounts {
                required: 1,
                optional: 1,
                unsupported: 1
            }
        );
        assert_eq!(
            report.server,
            SideCounts {
                required: 1,
                optional: 1,
                unsupported: 1
            }
        );
        assert_eq!(report.dependencies["future-loader"], "unknown");
        assert_eq!(report.dependencies["minecraft"], "1.21.1");
    }

    #[test]
    fn rejects_unsupported_and_malformed_metadata() {
        let mut value = fixture();
        value["formatVersion"] = json!(2);
        assert_eq!(inspect(&value), Err(InspectError::UnsupportedFormat));
        value["formatVersion"] = json!(1);
        value["files"][0]["env"] = json!({"client":"maybe","server":"required"});
        assert_eq!(inspect(&value), Err(InspectError::InvalidJson));
        value["files"] = json!([]);
        value["dependencies"] = json!({});
        assert_eq!(inspect(&value), Err(InspectError::InvalidMetadata));
        assert_eq!(inspect_index(b"{"), Err(InspectError::InvalidJson));
    }

    #[test]
    fn bounds_input_file_count_and_size_sum() {
        assert_eq!(
            inspect_index(&vec![b' '; MAX_INDEX_BYTES + 1]),
            Err(InspectError::InputTooLarge)
        );
        let mut value = fixture();
        value["files"] = json!([{"fileSize":u64::MAX},{"fileSize":1}]);
        assert_eq!(inspect(&value), Err(InspectError::DownloadSizeOverflow));
        value["files"] = json!(vec![json!({"fileSize":0}); MAX_FILES + 1]);
        assert_eq!(inspect(&value), Err(InspectError::TooManyFiles));
    }
}

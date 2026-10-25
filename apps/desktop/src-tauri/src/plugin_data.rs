// SPDX-License-Identifier: MIT
//! Host-owned plugin JSON sidecars; caller scope and keys never become paths.
use cobble_plugin_api::{PermissionNode, PluginManifest};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_DOCUMENT: usize = 64 * 1024;
const MAX_PLUGIN: usize = 2 * 1024 * 1024;
static WRITES: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadRequest {
    pub scope: Scope,
    pub key: String,
}

fn required_revision<'de, D: Deserializer<'de>>(input: D) -> Result<Option<String>, D::Error> {
    Option::deserialize(input)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WriteRequest {
    pub scope: Scope,
    pub key: String,
    pub value: Value,
    #[serde(deserialize_with = "required_revision")]
    pub expected_revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveRequest {
    pub scope: Scope,
    pub key: String,
    #[serde(deserialize_with = "required_revision")]
    pub expected_revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub value: Value,
    pub revision: Option<String>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            value: Value::Null,
            revision: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataError {
    InvalidKey,
    InvalidScope,
    InvalidRevision,
    Conflict,
    TooLarge,
    Quota,
    Corrupt,
    Unavailable,
    Revoked,
}

impl DataError {
    fn code(self) -> &'static str {
        match self {
            Self::InvalidKey => "PLUGIN_DATA_INVALID_KEY",
            Self::InvalidScope => "PLUGIN_DATA_INVALID_SCOPE",
            Self::InvalidRevision => "PLUGIN_DATA_INVALID_REVISION",
            Self::Conflict => "PLUGIN_DATA_CONFLICT",
            Self::TooLarge => "PLUGIN_DATA_TOO_LARGE",
            Self::Quota => "PLUGIN_DATA_QUOTA",
            Self::Corrupt => "PLUGIN_DATA_CORRUPT",
            Self::Unavailable => "PLUGIN_DATA_UNAVAILABLE",
            Self::Revoked => "PLUGIN_VIEW_REVOKED",
        }
    }
}

impl From<DataError> for String {
    fn from(error: DataError) -> Self {
        error.code().into()
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    schema_version: u32,
    documents: BTreeMap<String, Document>,
}

pub fn permitted(manifest: &PluginManifest) -> Result<(), String> {
    if manifest
        .permissions
        .as_deref()
        .unwrap_or_default()
        .contains(&PermissionNode::PrivateStorage {})
    {
        Ok(())
    } else {
        Err("PLUGIN_PERMISSION_DENIED".into())
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

pub fn authorize_scope(bound: Option<&str>, scope: &Scope) -> Result<(), String> {
    if scope.game_id.as_deref().is_some_and(|id| !identifier(id))
        || scope.world_id.as_deref().is_some_and(|id| !identifier(id))
        || (scope.world_id.is_some() && scope.game_id.is_none())
        || scope.server_id.as_deref().is_some_and(|id| {
            id.len() != 64
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        })
        || (scope.server_id.is_some() && (scope.game_id.is_none() || scope.world_id.is_some()))
    {
        return Err(DataError::InvalidScope.into());
    }
    if let (Some(bound), Some(game)) = (bound, scope.game_id.as_deref())
        && bound != game
    {
        return Err("PLUGIN_GAME_SCOPE".into());
    }
    Ok(())
}

fn slot(scope: &Scope, key: &str) -> Result<String, DataError> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
    {
        return Err(DataError::InvalidKey);
    }
    authorize_scope(None, scope).map_err(|_| DataError::InvalidScope)?;
    let bytes = serde_json::to_vec(&(scope, key)).map_err(|_| DataError::InvalidScope)?;
    Ok(cobble_plugin_host::digest(&bytes))
}

fn revision(value: Option<&str>) -> Result<(), DataError> {
    if value.is_some_and(|v| {
        v.len() != 64
            || !v
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    }) {
        Err(DataError::InvalidRevision)
    } else {
        Ok(())
    }
}

fn ordinary(path: &Path, directory: bool) -> Result<(), DataError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| DataError::Unavailable)?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked
        || if directory {
            !metadata.is_dir()
        } else {
            !metadata.is_file()
        }
    {
        Err(DataError::Unavailable)
    } else {
        Ok(())
    }
}

fn path(directory: &Path, plugin_id: &str) -> Result<PathBuf, DataError> {
    super::valid_id(plugin_id).map_err(|_| DataError::Unavailable)?;
    Ok(directory.join(format!("{plugin_id}.json")))
}

fn load(directory: &Path, plugin_id: &str) -> Result<Stored, DataError> {
    let path = path(directory, plugin_id)?;
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Stored {
                schema_version: 1,
                ..Stored::default()
            });
        }
        Err(_) => return Err(DataError::Unavailable),
        Ok(_) => ordinary(directory, true)?,
    }
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Stored {
                schema_version: 1,
                ..Stored::default()
            });
        }
        Err(_) => return Err(DataError::Unavailable),
        Ok(_) => ordinary(&path, false)?,
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| DataError::Unavailable)?
        .take(MAX_PLUGIN as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| DataError::Unavailable)?;
    if bytes.len() > MAX_PLUGIN {
        return Err(DataError::Quota);
    }
    let stored: Stored = serde_json::from_slice(&bytes).map_err(|_| DataError::Corrupt)?;
    if stored.schema_version != 1
        || stored.documents.iter().any(|(slot, doc)| {
            revision(Some(slot)).is_err()
                || doc.revision.is_none()
                || revision(doc.revision.as_deref()).is_err()
                || serde_json::to_vec(&doc.value).map_or(true, |bytes| bytes.len() > MAX_DOCUMENT)
        })
    {
        return Err(DataError::Corrupt);
    }
    Ok(stored)
}

fn persist(
    directory: &Path,
    plugin_id: &str,
    stored: &Stored,
    cancelled: &AtomicBool,
) -> Result<(), DataError> {
    let bytes = serde_json::to_vec(stored).map_err(|_| DataError::Corrupt)?;
    if bytes.len() > MAX_PLUGIN {
        return Err(DataError::Quota);
    }
    let path = path(directory, plugin_id)?;
    fs::create_dir_all(directory).map_err(|_| DataError::Unavailable)?;
    ordinary(directory, true)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| DataError::Unavailable)?;
    temporary
        .write_all(&bytes)
        .map_err(|_| DataError::Unavailable)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| DataError::Unavailable)?;
    if cancelled.load(Ordering::Acquire) {
        return Err(DataError::Revoked);
    }
    temporary
        .persist(path)
        .map_err(|_| DataError::Unavailable)?;
    Ok(())
}

fn read(
    directory: &Path,
    plugin_id: &str,
    scope: &Scope,
    key: &str,
) -> Result<Document, DataError> {
    let slot = slot(scope, key)?;
    Ok(load(directory, plugin_id)?
        .documents
        .remove(&slot)
        .unwrap_or_default())
}

fn write(
    directory: &Path,
    plugin_id: &str,
    request: WriteRequest,
    cancelled: &AtomicBool,
) -> Result<Document, DataError> {
    let slot = slot(&request.scope, &request.key)?;
    revision(request.expected_revision.as_deref())?;
    if serde_json::to_vec(&request.value)
        .map_err(|_| DataError::Corrupt)?
        .len()
        > MAX_DOCUMENT
    {
        return Err(DataError::TooLarge);
    }
    let mut stored = load(directory, plugin_id)?;
    if stored
        .documents
        .get(&slot)
        .and_then(|doc| doc.revision.as_deref())
        != request.expected_revision.as_deref()
    {
        return Err(DataError::Conflict);
    }
    let mut nonce = [0; 32];
    getrandom::fill(&mut nonce).map_err(|_| DataError::Unavailable)?;
    let document = Document {
        value: request.value,
        revision: Some(cobble_plugin_host::digest(&nonce)),
    };
    stored.documents.insert(slot, document.clone());
    persist(directory, plugin_id, &stored, cancelled)?;
    Ok(document)
}

fn remove(
    directory: &Path,
    plugin_id: &str,
    request: RemoveRequest,
    cancelled: &AtomicBool,
) -> Result<(), DataError> {
    let slot = slot(&request.scope, &request.key)?;
    revision(request.expected_revision.as_deref())?;
    let mut stored = load(directory, plugin_id)?;
    if stored
        .documents
        .get(&slot)
        .and_then(|doc| doc.revision.as_deref())
        != request.expected_revision.as_deref()
    {
        return Err(DataError::Conflict);
    }
    if stored.documents.remove(&slot).is_some() {
        persist(directory, plugin_id, &stored, cancelled)?;
    }
    Ok(())
}

async fn run<T: Send + 'static>(
    auth: &super::plugin_views::ViewAuthorization,
    scope: &Scope,
    operation: impl FnOnce(&Path, &str, &AtomicBool) -> Result<T, DataError> + Send + 'static,
) -> Result<T, String> {
    permitted(&auth.manifest)?;
    authorize_scope(auth.instance_id.as_deref(), scope)?;
    let plugin_id = auth.manifest.id.clone();
    let cancelled = auth.cancel.clone();
    let directory = super::root("plugin-data").map_err(|_| DataError::Unavailable)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = WRITES.lock().map_err(|_| DataError::Unavailable)?;
        if cancelled.load(Ordering::Acquire) {
            return Err("PLUGIN_VIEW_REVOKED".into());
        }
        operation(&directory, &plugin_id, &cancelled).map_err(String::from)
    })
    .await
    .map_err(|_| DataError::Unavailable)?
}

pub async fn read_for_frontend(
    auth: &super::plugin_views::ViewAuthorization,
    request: ReadRequest,
) -> Result<Document, String> {
    run(auth, &request.scope.clone(), move |directory, id, _| {
        read(directory, id, &request.scope, &request.key)
    })
    .await
}

pub async fn write_for_frontend(
    auth: &super::plugin_views::ViewAuthorization,
    request: WriteRequest,
) -> Result<Document, String> {
    run(
        auth,
        &request.scope.clone(),
        move |directory, id, cancelled| write(directory, id, request, cancelled),
    )
    .await
}

pub async fn remove_for_frontend(
    auth: &super::plugin_views::ViewAuthorization,
    request: RemoveRequest,
) -> Result<(), String> {
    run(
        auth,
        &request.scope.clone(),
        move |directory, id, cancelled| remove(directory, id, request, cancelled),
    )
    .await
}

#[cfg(test)]
#[path = "plugin_data_tests.rs"]
mod tests;

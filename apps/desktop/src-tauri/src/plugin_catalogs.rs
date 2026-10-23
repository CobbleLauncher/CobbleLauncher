// SPDX-License-Identifier: MIT
//! Explicit catalog subscriptions. Listing is disk-only; refresh uses the public broker.
use cobble_net::{Broker, BrokerConfig, NetError, PublicMetadataCache};
use cobble_plugin_host::catalog::{
    CatalogSnapshot, CatalogSource, CatalogStore, MAX_CATALOG_BYTES,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

const CACHE_PARTITION: &str = "plugin-catalogs-v1";
const OFFICIAL_CATALOG_NAME: &str = "CobbleLauncher Official";
const OFFICIAL_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/CobbleLauncher/CobbleLauncher/main/catalog/index.json";
#[derive(Default)]
struct CatalogState {
    errors: HashMap<String, String>,
    revisions: HashMap<String, u64>,
    sequence: u64,
}
impl CatalogState {
    fn advance(&mut self, id: &str) -> u64 {
        self.sequence += 1;
        self.revisions.insert(id.into(), self.sequence);
        self.sequence
    }
}
static STATE: OnceLock<Mutex<CatalogState>> = OnceLock::new();
static STORE_INITIALIZATION: OnceLock<Mutex<()>> = OnceLock::new();
static STORE_INITIALIZED: AtomicBool = AtomicBool::new(false);
fn state() -> &'static Mutex<CatalogState> {
    STATE.get_or_init(Default::default)
}
pub(super) fn store() -> Result<CatalogStore, String> {
    let root = dirs::data_local_dir().ok_or("无法找到数据目录")?;
    let store = CatalogStore::new(root.join("CobbleLauncher/plugin-catalogs"));
    if !STORE_INITIALIZED.load(Ordering::Acquire) {
        let _initialization = STORE_INITIALIZATION
            .get_or_init(Default::default)
            .lock()
            .map_err(|error| error.to_string())?;
        if !STORE_INITIALIZED.load(Ordering::Relaxed) {
            store
                .add_source_once(OFFICIAL_CATALOG_NAME, OFFICIAL_CATALOG_URL)
                .map_err(|error| error.to_string())?;
            STORE_INITIALIZED.store(true, Ordering::Release);
        }
    }
    Ok(store)
}
fn broker() -> Result<Broker, String> {
    static BROKER: OnceLock<Result<Broker, NetError>> = OnceLock::new();
    BROKER
        .get_or_init(|| Broker::new(BrokerConfig::default()))
        .clone()
        .map_err(|error| error.to_string())
}
#[derive(Serialize)]
pub struct CatalogView {
    source: CatalogSource,
    snapshot: Option<CatalogSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
fn views(store: &CatalogStore, state: &CatalogState) -> Result<Vec<CatalogView>, String> {
    store
        .list_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|source| {
            let (snapshot, read_error) = match store.read_snapshot(&source.id) {
                Ok(snapshot) => (snapshot, None),
                Err(error) => (None, Some(error.to_string())),
            };
            let error = state.errors.get(&source.id).cloned().or(read_error);
            Ok(CatalogView {
                source,
                snapshot,
                error,
            })
        })
        .collect()
}

#[tauri::command]
pub fn list_plugin_catalogs() -> Result<Vec<CatalogView>, String> {
    let state = state().lock().map_err(|error| error.to_string())?;
    let store = store()?;
    views(&store, &state)
}
#[tauri::command]
pub async fn add_plugin_catalog(name: String, url: String) -> Result<Vec<CatalogView>, String> {
    let source = {
        let _state = state().lock().map_err(|error| error.to_string())?;
        store()?
            .add_source(name.trim(), url.trim())
            .map_err(|error| error.to_string())?
    };
    refresh_plugin_catalog(source.id).await
}
#[tauri::command]
pub fn set_plugin_catalog_enabled(id: String, enabled: bool) -> Result<Vec<CatalogView>, String> {
    let mut state = state().lock().map_err(|error| error.to_string())?;
    let store = store()?;
    store
        .set_enabled(&id, enabled)
        .map_err(|error| error.to_string())?;
    state.advance(&id);
    state.errors.remove(&id);
    views(&store, &state)
}
#[tauri::command]
pub fn remove_plugin_catalog(id: String) -> Result<Vec<CatalogView>, String> {
    let mut state = state().lock().map_err(|error| error.to_string())?;
    let store = store()?;
    store
        .remove_source(&id)
        .map_err(|error| error.to_string())?;
    state.advance(&id);
    state.errors.remove(&id);
    views(&store, &state)
}
#[tauri::command]
pub async fn refresh_plugin_catalog(id: String) -> Result<Vec<CatalogView>, String> {
    let (source, revision) = {
        let mut state = state().lock().map_err(|error| error.to_string())?;
        let source = store()?
            .list_sources()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|source| source.id == id)
            .ok_or("找不到这个插件目录")?;
        if !source.enabled {
            return Err("请先启用这个插件目录".into());
        }
        let revision = state.advance(&id);
        (source, revision)
    };
    let result = fetch(&source.url).await;
    let mut state = state().lock().map_err(|error| error.to_string())?;
    let store = store()?;
    finish_refresh(&store, &mut state, &id, revision, result);
    views(&store, &state)
}
async fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let limit = MAX_CATALOG_BYTES as u64;
    if let Some(cache) = PublicMetadataCache::shared() {
        cache
            .invalidate(url, CACHE_PARTITION, limit)
            .await
            .map_err(|error| error.to_string())?;
    }
    let response = broker()?
        .get_public_metadata(url, CACHE_PARTITION, limit)
        .await
        .map_err(|error| error.to_string())?;
    if response.metadata.status != 200 {
        return Err(format!(
            "插件目录请求失败（HTTP {}）",
            response.metadata.status
        ));
    }
    Ok(response.body)
}
fn finish_refresh(
    store: &CatalogStore,
    state: &mut CatalogState,
    id: &str,
    revision: u64,
    result: Result<Vec<u8>, String>,
) {
    if state.revisions.get(id) != Some(&revision) {
        return;
    }
    let result = result.and_then(|bytes| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs();
        store
            .save_snapshot(id, &bytes, now)
            .map_err(|error| error.to_string())?;
        Ok(())
    });
    match result {
        Ok(()) => {
            state.errors.remove(id);
        }
        Err(error) => {
            state.errors.insert(id.into(), error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, CatalogStore, CatalogSource, CatalogState) {
        let root = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(root.path().to_path_buf());
        let source = store
            .add_source("Fixture", "https://catalog.example/index.json")
            .unwrap();
        store
            .save_snapshot(
                &source.id,
                br#"{"schema_version":1,"name":"Cached","plugins":[]}"#,
                1,
            )
            .unwrap();
        (root, store, source, CatalogState::default())
    }
    #[test]
    fn failed_refresh_keeps_snapshot_and_reports_error() {
        let (_root, store, source, mut state) = fixture();
        let revision = state.advance(&source.id);
        finish_refresh(
            &store,
            &mut state,
            &source.id,
            revision,
            Err("network unavailable".into()),
        );
        let result = views(&store, &state).unwrap();
        assert_eq!(result[0].snapshot.as_ref().unwrap().index.name, "Cached");
        assert_eq!(result[0].error.as_deref(), Some("network unavailable"));
        finish_refresh(
            &store,
            &mut state,
            &source.id,
            revision,
            Ok(b"invalid JSON".to_vec()),
        );
        assert_eq!(
            store
                .read_snapshot(&source.id)
                .unwrap()
                .unwrap()
                .retrieved_at,
            1
        );
    }
    #[test]
    fn stale_refresh_cannot_replace_snapshot_or_recreate_removed_source() {
        let (_root, store, source, mut state) = fixture();
        let stale = state.advance(&source.id);
        let newer = state.advance(&source.id);
        let valid = br#"{"schema_version":1,"name":"Updated","plugins":[]}"#.to_vec();
        finish_refresh(&store, &mut state, &source.id, stale, Ok(valid.clone()));
        assert_eq!(
            store.read_snapshot(&source.id).unwrap().unwrap().index.name,
            "Cached"
        );
        finish_refresh(&store, &mut state, &source.id, newer, Ok(valid.clone()));
        assert_eq!(
            store.read_snapshot(&source.id).unwrap().unwrap().index.name,
            "Updated"
        );
        store.remove_source(&source.id).unwrap();
        state.advance(&source.id);
        finish_refresh(&store, &mut state, &source.id, newer, Ok(valid));
        assert!(views(&store, &state).unwrap().is_empty());
    }

    #[test]
    fn disabling_during_refresh_preserves_cached_catalog() {
        let (_root, store, source, mut state) = fixture();
        let pending = state.advance(&source.id);
        store.set_enabled(&source.id, false).unwrap();
        state.advance(&source.id);
        finish_refresh(
            &store,
            &mut state,
            &source.id,
            pending,
            Ok(br#"{"schema_version":1,"name":"Ignored","plugins":[]}"#.to_vec()),
        );
        let result = views(&store, &state).unwrap();
        assert!(!result[0].source.enabled);
        assert_eq!(result[0].snapshot.as_ref().unwrap().index.name, "Cached");
        assert!(result[0].error.is_none());
    }
}

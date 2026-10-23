// SPDX-License-Identifier: MIT
use super::{CatalogIndex, MAX_CATALOG_BYTES, invalid, parse_index, validate_url};
use cobble_plugin_api::PluginError;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf};

const DEFAULT_SOURCE_MARKER: &str = "default-source-initialized-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSource {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogSnapshot {
    pub source_id: String,
    pub source_url: String,
    /// Unix timestamp in seconds, supplied by the network adapter after retrieval.
    pub retrieved_at: u64,
    pub index: CatalogIndex,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sources {
    schema_version: u32,
    sources: Vec<CatalogSource>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSnapshot {
    schema_version: u32,
    source_id: String,
    source_url: String,
    retrieved_at: u64,
    raw_json: String,
}

/// Caller supplies a private state directory and serializes mutations.
pub struct CatalogStore {
    root: PathBuf,
}
impl CatalogStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn list_sources(&self) -> Result<Vec<CatalogSource>, PluginError> {
        let bytes = match super::super::read_regular(&self.root, "sources.json", 512 * 1024) {
            Ok(bytes) => bytes,
            Err(_)
                if !self
                    .root
                    .join("sources.json")
                    .try_exists()
                    .map_err(crate::io_error)? =>
            {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };
        let data: Sources = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("Invalid source config: {error}")))?;
        if data.schema_version != 1 || data.sources.len() > 32 {
            return Err(invalid("Unsupported source config or more than 32 sources"));
        }
        let mut ids = std::collections::HashSet::new();
        for source in &data.sources {
            validate_source(source)?;
            if !ids.insert(&source.id) {
                return Err(invalid("Duplicate catalog source"));
            }
        }
        Ok(data.sources)
    }
    pub fn add_source(&self, name: &str, url: &str) -> Result<CatalogSource, PluginError> {
        let source = CatalogSource {
            id: crate::digest(url.as_bytes()),
            name: name.into(),
            url: url.into(),
            enabled: true,
        };
        validate_source(&source)?;
        let mut sources = self.list_sources()?;
        if sources.iter().any(|existing| existing.id == source.id) {
            return Err(PluginError::new(
                "PLUGIN_SOURCE_EXISTS",
                "This exact catalog URL is already subscribed",
            ));
        }
        if sources.len() >= 32 {
            return Err(invalid("At most 32 catalog sources are supported"));
        }
        sources.push(source.clone());
        self.write_sources(sources)?;
        Ok(source)
    }
    /// Adds a built-in source once, including for stores that already have user sources.
    /// The marker remains after source removal so user choices are never undone.
    pub fn add_source_once(
        &self,
        name: &str,
        url: &str,
    ) -> Result<Option<CatalogSource>, PluginError> {
        match fs::symlink_metadata(self.root.join(DEFAULT_SOURCE_MARKER)) {
            Ok(_) => Ok(None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let existing = self.list_sources()?;
                let added = if existing.iter().any(|source| source.url == url) {
                    None
                } else {
                    Some(self.add_source(name, url)?)
                };
                self.atomic_write(DEFAULT_SOURCE_MARKER, b"1\n")?;
                Ok(added)
            }
            Err(error) => Err(crate::io_error(error)),
        }
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<CatalogSource, PluginError> {
        let mut sources = self.list_sources()?;
        let source = sources
            .iter_mut()
            .find(|source| source.id == id)
            .ok_or_else(missing)?;
        source.enabled = enabled;
        let result = source.clone();
        self.write_sources(sources)?;
        Ok(result)
    }
    pub fn remove_source(&self, id: &str) -> Result<(), PluginError> {
        let mut sources = self.list_sources()?;
        let index = sources
            .iter()
            .position(|source| source.id == id)
            .ok_or_else(missing)?;
        sources.remove(index);
        self.write_sources(sources)?;
        match fs::remove_file(self.root.join(format!("{id}.json"))) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(crate::io_error(error)),
        }
    }
    pub fn read_snapshot(&self, id: &str) -> Result<Option<CatalogSnapshot>, PluginError> {
        let source = self.source(id)?;
        let file = format!("{id}.json");
        let bytes =
            match super::super::read_regular(&self.root, &file, MAX_CATALOG_BYTES * 6 + 4096) {
                Ok(bytes) => bytes,
                Err(_)
                    if !self
                        .root
                        .join(&file)
                        .try_exists()
                        .map_err(crate::io_error)? =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
        let stored: StoredSnapshot = serde_json::from_slice(&bytes)
            .map_err(|error| invalid(format!("Invalid cached source: {error}")))?;
        if stored.schema_version != 1
            || stored.source_id != source.id
            || stored.source_url != source.url
        {
            return Err(invalid(
                "Cached catalog does not match its subscribed source",
            ));
        }
        Ok(Some(CatalogSnapshot {
            source_id: stored.source_id,
            source_url: stored.source_url,
            retrieved_at: stored.retrieved_at,
            index: parse_index(stored.raw_json.as_bytes())?,
        }))
    }
    /// A refresh only replaces a cache after all entries validate. Network failures
    /// must be handled by callers without invoking this method.
    pub fn save_snapshot(
        &self,
        id: &str,
        raw: &[u8],
        retrieved_at: u64,
    ) -> Result<CatalogSnapshot, PluginError> {
        let source = self.source(id)?;
        if !source.enabled {
            return Err(PluginError::new(
                "PLUGIN_SOURCE_DISABLED",
                "Enable this source before refreshing",
            ));
        }
        let index = parse_index(raw)?;
        let stored = StoredSnapshot {
            schema_version: 1,
            source_id: source.id.clone(),
            source_url: source.url.clone(),
            retrieved_at,
            raw_json: String::from_utf8(raw.to_vec())
                .map_err(|_| invalid("Catalog must be UTF-8"))?,
        };
        self.atomic_write(
            &format!("{id}.json"),
            &serde_json::to_vec(&stored).map_err(crate::io_error)?,
        )?;
        Ok(CatalogSnapshot {
            source_id: source.id,
            source_url: source.url,
            retrieved_at,
            index,
        })
    }
    fn source(&self, id: &str) -> Result<CatalogSource, PluginError> {
        self.list_sources()?
            .into_iter()
            .find(|source| source.id == id)
            .ok_or_else(missing)
    }
    fn write_sources(&self, sources: Vec<CatalogSource>) -> Result<(), PluginError> {
        self.atomic_write(
            "sources.json",
            &serde_json::to_vec_pretty(&Sources {
                schema_version: 1,
                sources,
            })
            .map_err(crate::io_error)?,
        )
    }
    fn atomic_write(&self, name: &str, bytes: &[u8]) -> Result<(), PluginError> {
        fs::create_dir_all(&self.root).map_err(crate::io_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.root).map_err(crate::io_error)?;
        temporary.write_all(bytes).map_err(crate::io_error)?;
        temporary.as_file().sync_all().map_err(crate::io_error)?;
        temporary
            .persist(self.root.join(name))
            .map_err(crate::io_error)?;
        Ok(())
    }
}
fn missing() -> PluginError {
    PluginError::new("PLUGIN_SOURCE_NOT_FOUND", "Catalog source not found")
}
fn validate_source(source: &CatalogSource) -> Result<(), PluginError> {
    validate_url(&source.url)?;
    if source.id != crate::digest(source.url.as_bytes())
        || source.name.trim().is_empty()
        || source.name.len() > 120
    {
        return Err(invalid(
            "Source requires its exact URL digest and a name up to 120 bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_crud_and_exact_url_cache_isolation() {
        let directory = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(directory.path().into());
        assert!(store.list_sources().unwrap().is_empty());
        let a = store
            .add_source("A", "https://example.invalid/a.json")
            .unwrap();
        let b = store
            .add_source("B", "https://example.invalid/b.json")
            .unwrap();
        assert_ne!(a.id, b.id);
        assert!(store.add_source("Again", &a.url).is_err());
        let raw = serde_json::to_vec(&super::super::tests::fixture()).unwrap();
        let snapshot = store.save_snapshot(&a.id, &raw, 123).unwrap();
        assert_eq!(store.read_snapshot(&a.id).unwrap(), Some(snapshot.clone()));
        assert!(store.read_snapshot(&b.id).unwrap().is_none());
        assert!(store.save_snapshot(&a.id, b"broken", 456).is_err());
        assert_eq!(store.read_snapshot(&a.id).unwrap(), Some(snapshot));
        store.set_enabled(&a.id, false).unwrap();
        assert!(store.save_snapshot(&a.id, &raw, 789).is_err());
        store.remove_source(&a.id).unwrap();
        assert_eq!(store.list_sources().unwrap(), vec![b]);
        assert!(store.read_snapshot(&a.id).is_err());
        assert!(store.remove_source("../../outside").is_err());
    }
    #[test]
    fn refuses_snapshot_copied_between_sources() {
        let directory = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(directory.path().into());
        let a = store
            .add_source("A", "https://example.invalid/a.json")
            .unwrap();
        let b = store
            .add_source("B", "https://example.invalid/b.json")
            .unwrap();
        store
            .save_snapshot(
                &a.id,
                &serde_json::to_vec(&super::super::tests::fixture()).unwrap(),
                1,
            )
            .unwrap();
        fs::copy(
            directory.path().join(format!("{}.json", a.id)),
            directory.path().join(format!("{}.json", b.id)),
        )
        .unwrap();
        assert!(store.read_snapshot(&b.id).is_err());
    }

    #[test]
    fn default_source_is_added_once_alongside_custom_sources_and_user_removal_persists() {
        let directory = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(directory.path().into());
        let custom = store
            .add_source("Custom", "https://community.example/index.json")
            .unwrap();
        let first = store
            .add_source_once("Official", "https://catalog.example/index.json")
            .unwrap()
            .unwrap();
        assert!(
            store
                .add_source_once("Official", "https://catalog.example/index.json")
                .unwrap()
                .is_none()
        );
        store.set_enabled(&first.id, false).unwrap();
        assert!(
            store
                .add_source_once("Official", "https://catalog.example/index.json")
                .unwrap()
                .is_none()
        );
        let sources = store.list_sources().unwrap();
        assert_eq!(sources.len(), 2);
        assert!(
            !sources
                .iter()
                .find(|source| source.id == first.id)
                .unwrap()
                .enabled
        );
        store.remove_source(&first.id).unwrap();
        assert_eq!(store.list_sources().unwrap(), vec![custom.clone()]);
        assert!(
            store
                .add_source_once("Official", "https://catalog.example/index.json")
                .unwrap()
                .is_none()
        );
        assert_eq!(store.list_sources().unwrap(), vec![custom]);
    }

    #[test]
    fn existing_official_url_is_marked_without_creating_a_duplicate() {
        let directory = tempfile::tempdir().unwrap();
        let store = CatalogStore::new(directory.path().into());
        let existing = store
            .add_source("My official mirror", "https://catalog.example/index.json")
            .unwrap();
        assert!(
            store
                .add_source_once("Official", "https://catalog.example/index.json")
                .unwrap()
                .is_none()
        );
        store.remove_source(&existing.id).unwrap();
        assert!(
            store
                .add_source_once("Official", "https://catalog.example/index.json")
                .unwrap()
                .is_none()
        );
        assert!(store.list_sources().unwrap().is_empty());
    }
}

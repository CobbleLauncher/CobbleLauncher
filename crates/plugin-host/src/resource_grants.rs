// SPDX-License-Identifier: MIT
//! Host-side selected-folder capabilities. No guest ABI or renderer command is implied.
//! The trusted caller must obtain explicit selection before granting a directory.
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::{Dir, File, OpenOptions};
use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

const MAX_HANDLES: usize = 256;
const MAX_READ: u32 = 65_536;
const TOTAL_READ: u64 = 8 * 1024 * 1024;

/// Includes the installed manifest digest so an update cannot inherit handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginIdentity {
    pub publisher: String,
    pub plugin_id: String,
    pub manifest_sha256: String,
}

/// Opaque and intentionally not serializable until an invocation adapter exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceHandle([u8; 16]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantError {
    Denied,
    Limit,
    InvalidRange,
    Io,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
}

#[derive(Debug)]
pub struct FolderEntry {
    /// A display label only, never an input path accepted by this API.
    pub name: String,
    pub kind: EntryKind,
    pub handle: ResourceHandle,
}

/// A one-level selection; skipped names and ambient paths are never returned.
#[derive(Debug)]
pub struct FolderListing {
    pub entries: Vec<FolderEntry>,
    pub skipped: u32,
    pub truncated: bool,
}

enum Resource {
    Directory(Dir),
    File(File),
}
struct Entry {
    owner: PluginIdentity,
    grant: ResourceHandle,
    resource: Resource,
}

/// Invocation-scoped registry. Drop or revoke it when the invocation ends.
#[derive(Default)]
pub struct FolderGrants {
    entries: HashMap<ResourceHandle, Entry>,
    bytes_read: u64,
}

impl FolderGrants {
    fn token(&self) -> Result<ResourceHandle, GrantError> {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|_| GrantError::Io)?;
        let token = ResourceHandle(bytes);
        if self.entries.contains_key(&token) {
            return Err(GrantError::Io);
        }
        Ok(token)
    }

    /// Host-only: `selected` comes from a trusted native picker, never a plugin path.
    pub fn grant_selected(
        &mut self,
        owner: PluginIdentity,
        selected: &Path,
    ) -> Result<ResourceHandle, GrantError> {
        if self.entries.len() >= MAX_HANDLES {
            return Err(GrantError::Limit);
        }
        let metadata = std::fs::symlink_metadata(selected).map_err(|_| GrantError::Io)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(GrantError::Denied);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(GrantError::Denied);
            }
        }
        let directory = Dir::open_ambient_dir(selected, cap_std::ambient_authority())
            .map_err(|_| GrantError::Io)?;
        let handle = self.token()?;
        self.entries.insert(
            handle,
            Entry {
                owner,
                grant: handle,
                resource: Resource::Directory(directory),
            },
        );
        Ok(handle)
    }

    /// Lists one level, capped by the invocation's remaining handle budget.
    /// Links and special files are refused; no recursive ambient-path walk occurs.
    pub fn list(
        &mut self,
        owner: &PluginIdentity,
        handle: ResourceHandle,
    ) -> Result<Vec<FolderEntry>, GrantError> {
        let entry = self
            .entries
            .get(&handle)
            .filter(|entry| &entry.owner == owner)
            .ok_or(GrantError::Denied)?;
        let Resource::Directory(directory) = &entry.resource else {
            return Err(GrantError::Denied);
        };
        let grant = entry.grant;
        let mut pending = Vec::new();
        for item in directory.entries().map_err(|_| GrantError::Io)? {
            if self.entries.len() + pending.len() >= MAX_HANDLES {
                return Err(GrantError::Limit);
            }
            let item = item.map_err(|_| GrantError::Io)?;
            let name = item.file_name();
            let label = name
                .to_str()
                .filter(|name| name.len() <= 255)
                .ok_or(GrantError::Denied)?
                .to_owned();
            let metadata = directory
                .symlink_metadata(&name)
                .map_err(|_| GrantError::Io)?;
            reject_link(&metadata)?;
            let (kind, resource) = if metadata.is_dir() {
                (
                    EntryKind::Directory,
                    Resource::Directory(
                        directory
                            .open_dir_nofollow(&name)
                            .map_err(|_| GrantError::Denied)?,
                    ),
                )
            } else if metadata.is_file() {
                let mut options = OpenOptions::new();
                options.read(true).follow(FollowSymlinks::No);
                let file = directory
                    .open_with(&name, &options)
                    .map_err(|_| GrantError::Denied)?;
                let opened = file.metadata().map_err(|_| GrantError::Io)?;
                reject_link(&opened)?;
                if !opened.is_file() || opened.nlink() != 1 {
                    return Err(GrantError::Denied);
                }
                (EntryKind::File, Resource::File(file))
            } else {
                return Err(GrantError::Denied);
            };
            pending.push((label, kind, resource));
        }
        let mut output = Vec::new();
        for (name, kind, resource) in pending {
            let handle = self.token()?;
            self.entries.insert(
                handle,
                Entry {
                    owner: owner.clone(),
                    grant,
                    resource,
                },
            );
            output.push(FolderEntry { name, kind, handle });
        }
        output.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(output)
    }

    /// Select supported regular files without opening unrelated files or directories.
    /// Limits count all observed entries, including unsupported and unsafe entries.
    /// The trusted invocation adapter intersects `extensions` with its manifest grant.
    pub fn list_files_bounded(
        &mut self,
        owner: &PluginIdentity,
        root: &ResourceHandle,
        extensions: &[String],
        max_files: usize,
        max_scan: usize,
    ) -> Result<FolderListing, GrantError> {
        if !(1..=64).contains(&max_files)
            || !(1..=1024).contains(&max_scan)
            || extensions.is_empty()
            || extensions.len() > 16
            || extensions.iter().any(|extension| {
                extension.is_empty()
                    || extension.len() > 16
                    || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
            })
        {
            return Err(GrantError::Limit);
        }
        let entry = self
            .entries
            .get(root)
            .filter(|entry| &entry.owner == owner)
            .ok_or(GrantError::Denied)?;
        let Resource::Directory(directory) = &entry.resource else {
            return Err(GrantError::Denied);
        };
        let grant = entry.grant;
        let mut pending = Vec::new();
        let mut skipped = 0;
        let mut truncated = false;
        for (index, item) in directory.entries().map_err(|_| GrantError::Io)?.enumerate() {
            if index == max_scan {
                truncated = true;
                break;
            }
            let candidate = (|| {
                let item = item.ok()?;
                let name = item.file_name();
                let label = name
                    .to_str()
                    .filter(|label| label.len() <= 255 && !label.chars().any(char::is_control))?;
                let extension = Path::new(label).extension()?.to_str()?;
                if !extensions
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(extension))
                {
                    return None;
                }
                let metadata = directory.symlink_metadata(&name).ok()?;
                reject_link(&metadata).ok()?;
                if !metadata.is_file() || metadata.nlink() != 1 {
                    return None;
                }
                Some((name.clone(), label.to_owned()))
            })();
            let Some((name, label)) = candidate else {
                skipped += 1;
                continue;
            };
            if pending.len() >= max_files || self.entries.len() + pending.len() >= MAX_HANDLES {
                skipped += 1;
                truncated = true;
                continue;
            }
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No);
            let file = directory.open_with(&name, &options).ok().filter(|file| {
                file.metadata().ok().is_some_and(|metadata| {
                    reject_link(&metadata).is_ok() && metadata.is_file() && metadata.nlink() == 1
                })
            });
            if let Some(file) = file {
                pending.push((label, file));
            } else {
                skipped += 1;
            }
        }
        let mut entries = Vec::new();
        for (name, file) in pending {
            let handle = self.token()?;
            self.entries.insert(
                handle,
                Entry {
                    owner: owner.clone(),
                    grant,
                    resource: Resource::File(file),
                },
            );
            entries.push(FolderEntry {
                name,
                kind: EntryKind::File,
                handle,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(FolderListing {
            entries,
            skipped,
            truncated,
        })
    }

    pub fn read_at(
        &mut self,
        owner: &PluginIdentity,
        handle: ResourceHandle,
        offset: u64,
        maximum: u32,
    ) -> Result<Vec<u8>, GrantError> {
        if !(1..=MAX_READ).contains(&maximum) {
            return Err(GrantError::InvalidRange);
        }
        let entry = self
            .entries
            .get_mut(&handle)
            .filter(|entry| &entry.owner == owner)
            .ok_or(GrantError::Denied)?;
        let Resource::File(file) = &mut entry.resource else {
            return Err(GrantError::Denied);
        };
        let length = file.metadata().map_err(|_| GrantError::Io)?.len();
        if offset > length {
            return Err(GrantError::InvalidRange);
        }
        let count = u64::from(maximum).min(length - offset);
        if self
            .bytes_read
            .checked_add(count)
            .is_none_or(|total| total > TOTAL_READ)
        {
            return Err(GrantError::Limit);
        }
        file.seek(SeekFrom::Start(offset))
            .map_err(|_| GrantError::Io)?;
        let mut output = Vec::with_capacity(count as usize);
        file.take(count)
            .read_to_end(&mut output)
            .map_err(|_| GrantError::Io)?;
        self.bytes_read += output.len() as u64;
        Ok(output)
    }

    /// Revoking any handle invalidates its entire selection and all descendants.
    pub fn revoke(
        &mut self,
        owner: &PluginIdentity,
        handle: ResourceHandle,
    ) -> Result<(), GrantError> {
        let grant = self
            .entries
            .get(&handle)
            .filter(|entry| &entry.owner == owner)
            .ok_or(GrantError::Denied)?
            .grant;
        self.entries.retain(|_, entry| entry.grant != grant);
        Ok(())
    }
}

fn reject_link(metadata: &cap_std::fs::Metadata) -> Result<(), GrantError> {
    if metadata.file_type().is_symlink() {
        return Err(GrantError::Denied);
    }
    #[cfg(windows)]
    {
        use cap_std::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(GrantError::Denied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> PluginIdentity {
        PluginIdentity {
            publisher: "test".into(),
            plugin_id: "inspect".into(),
            manifest_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn selected_handles_are_bound_and_revoked_with_descendants() {
        let selected = tempfile::tempdir().unwrap();
        std::fs::create_dir(selected.path().join("nested")).unwrap();
        std::fs::write(selected.path().join("nested/data.txt"), b"selected bytes").unwrap();
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        let mut different = owner();
        different.manifest_sha256 = "b".repeat(64);
        assert_eq!(
            grants.list(&different, root).unwrap_err(),
            GrantError::Denied
        );
        let nested = grants.list(&owner(), root).unwrap().remove(0).handle;
        let file = grants.list(&owner(), nested).unwrap().remove(0).handle;
        assert_eq!(grants.read_at(&owner(), file, 9, 5).unwrap(), b"bytes");
        assert_eq!(
            grants.read_at(&owner(), file, 0, MAX_READ + 1),
            Err(GrantError::InvalidRange)
        );
        assert_eq!(
            grants.read_at(&owner(), file, 99, 1),
            Err(GrantError::InvalidRange)
        );
        grants.revoke(&owner(), root).unwrap();
        assert_eq!(
            grants.read_at(&owner(), file, 0, 1),
            Err(GrantError::Denied)
        );
        assert_eq!(
            grants.list(&owner(), nested).unwrap_err(),
            GrantError::Denied
        );
    }

    #[test]
    fn listing_and_reads_have_invocation_budgets() {
        let selected = tempfile::tempdir().unwrap();
        std::fs::write(selected.path().join("data"), vec![0; MAX_READ as usize]).unwrap();
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        let file = grants.list(&owner(), root).unwrap().remove(0).handle;
        for _ in 0..TOTAL_READ / u64::from(MAX_READ) {
            assert_eq!(
                grants.read_at(&owner(), file, 0, MAX_READ).unwrap().len(),
                MAX_READ as usize
            );
        }
        assert_eq!(grants.read_at(&owner(), file, 0, 1), Err(GrantError::Limit));
        while grants.entries.len() < MAX_HANDLES {
            grants.list(&owner(), root).unwrap();
        }
        assert_eq!(grants.list(&owner(), root).unwrap_err(), GrantError::Limit);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_escape() {
        let selected = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), selected.path().join("escape")).unwrap();
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        assert_eq!(grants.list(&owner(), root).unwrap_err(), GrantError::Denied);
        assert_eq!(grants.entries.len(), 1);
    }

    #[test]
    fn random_and_cross_registry_handles_do_not_authorize_access() {
        let selected = tempfile::tempdir().unwrap();
        let mut first = FolderGrants::default();
        let root = first.grant_selected(owner(), selected.path()).unwrap();
        let mut other = FolderGrants::default();
        assert_eq!(other.list(&owner(), root).unwrap_err(), GrantError::Denied);
        assert_eq!(
            first.list(&owner(), ResourceHandle([0; 16])).unwrap_err(),
            GrantError::Denied
        );
    }

    #[test]
    fn refuses_hard_links_to_outside_files() {
        let selected = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), b"outside").unwrap();
        std::fs::hard_link(outside.path().join("secret"), selected.path().join("alias")).unwrap();
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        assert_eq!(grants.list(&owner(), root).unwrap_err(), GrantError::Denied);
        assert_eq!(grants.entries.len(), 1);
    }

    #[test]
    fn bounded_file_selection_ignores_unrelated_files_without_allocating_handles() {
        let selected = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        for index in 0..300 {
            std::fs::write(
                selected.path().join(format!("artwork-{index}.png")),
                b"image",
            )
            .unwrap();
        }
        std::fs::create_dir(selected.path().join("nested")).unwrap();
        std::fs::write(selected.path().join("nested/private.json"), b"not selected").unwrap();
        std::fs::write(selected.path().join("settings.JSON"), b"{}").unwrap();
        std::fs::write(selected.path().join("settings.yaml"), b"enabled: true").unwrap();
        std::fs::write(outside.path().join("secret.json"), b"outside").unwrap();
        std::fs::hard_link(
            outside.path().join("secret.json"),
            selected.path().join("alias.json"),
        )
        .unwrap();
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        let listing = grants
            .list_files_bounded(&owner(), &root, &["json".into(), "yaml".into()], 64, 1024)
            .unwrap();
        assert_eq!(
            listing
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["settings.JSON", "settings.yaml"]
        );
        assert_eq!(listing.skipped, 302);
        assert!(!listing.truncated);
        assert_eq!(grants.entries.len(), 3);
        let selected_file = listing.entries[0].handle;
        assert_eq!(
            grants.read_at(&owner(), selected_file, 0, 10).unwrap(),
            b"{}"
        );
        grants.revoke(&owner(), root).unwrap();
        assert_eq!(
            grants.read_at(&owner(), selected_file, 0, 1),
            Err(GrantError::Denied)
        );
    }

    #[test]
    fn bounded_file_selection_reports_observed_skips_and_both_limits() {
        let selected = tempfile::tempdir().unwrap();
        for index in 0..10 {
            std::fs::write(selected.path().join(format!("config-{index}.json")), b"{}").unwrap();
        }
        let mut grants = FolderGrants::default();
        let root = grants.grant_selected(owner(), selected.path()).unwrap();
        let listing = grants
            .list_files_bounded(&owner(), &root, &["json".into()], 2, 4)
            .unwrap();
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.skipped, 2);
        assert!(listing.truncated);
        assert_eq!(grants.entries.len(), 3);
        let listing = grants
            .list_files_bounded(&owner(), &root, &["json".into()], 2, 10)
            .unwrap();
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.skipped, 8);
        assert!(listing.truncated);
        let mut different = owner();
        different.plugin_id = "other-plugin".into();
        assert_eq!(
            grants
                .list_files_bounded(&different, &root, &["json".into()], 2, 10)
                .unwrap_err(),
            GrantError::Denied
        );
        assert_eq!(
            grants
                .list_files_bounded(&owner(), &root, &["../json".into()], 2, 10)
                .unwrap_err(),
            GrantError::Limit
        );
    }
}

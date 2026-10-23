// SPDX-License-Identifier: MIT
//! Bounded, pinned official dependency resolution. No metadata is persisted.
use super::*;
use crate::{ModrinthFile, ModrinthPlan, ResolvedMod};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Serialize, Deserialize)]
pub struct RestrictedFile {
    pub project_id: String,
    pub file_id: String,
    pub name: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub website_url: Option<String>,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct InstallPlan {
    pub graph: ModrinthPlan,
    pub restricted: Vec<RestrictedFile>,
    #[serde(default)]
    pub incompatible: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackFileDestination {
    Mods,
    ResourcePack,
    ShaderPack,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ResolvedPackFile {
    pub project_id: String,
    pub file_id: String,
    pub file: ModrinthFile,
    pub destination: PackFileDestination,
    pub name: String,
    pub version_number: String,
    pub required_by: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PackInstallPlan {
    pub files: Vec<ResolvedPackFile>,
    pub restricted: Vec<RestrictedFile>,
    pub incompatible: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PackArchiveSelection {
    pub file: ModrinthFile,
    pub restricted: Option<RestrictedFile>,
    pub name: String,
    pub version: String,
}
#[derive(Deserialize)]
struct One<T> {
    data: T,
}

impl CurseForgeClient {
    pub async fn resolve_pack_archive(
        &self,
        project: u32,
        id: u32,
    ) -> Result<PackArchiveSelection, ApiError> {
        let metadata = self.project(project).await?;
        if metadata.class_id != 4471 {
            return Err(ApiError::UnsupportedClass);
        }
        let file = self.exact_file(project, id).await?;
        let sha1 = file
            .hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .filter(|hash| {
                hash.value.len() == 40 && hash.value.bytes().all(|b| b.is_ascii_hexdigit())
            })
            .ok_or(ApiError::InvalidResponse)?
            .value
            .to_ascii_lowercase();
        if file.file_length == 0
            || file.file_length > 512 * 1024 * 1024
            || file.file_name.len() > 240
            || file.file_name.contains(['/', '\\', ':'])
            || file.file_name.starts_with('.')
            || !file.file_name.to_ascii_lowercase().ends_with(".zip")
        {
            return Err(ApiError::InvalidResponse);
        }
        let (url, restricted) = artifact_route(&metadata, &file)?;
        Ok(PackArchiveSelection {
            file: ModrinthFile {
                url,
                filename: file.file_name,
                size: file.file_length,
                sha1,
                sha512: None,
            },
            restricted,
            name: metadata.title,
            version: file.display_name,
        })
    }
    async fn exact_file(&self, project: u32, id: u32) -> Result<RawFile, ApiError> {
        if project == 0 || id == 0 {
            return Err(ApiError::InvalidInput);
        }
        let one: One<RawFile> = self
            .get(
                Url::parse(&format!("{ORIGIN}/v1/mods/{project}/files/{id}"))
                    .map_err(|_| ApiError::InvalidInput)?,
            )
            .await?;
        if one.data.id != id || one.data.mod_id != project || one.data.game_id != MINECRAFT {
            return Err(ApiError::InvalidResponse);
        }
        Ok(one.data)
    }
    pub async fn project(&self, id: u32) -> Result<Project, ApiError> {
        if id == 0 {
            return Err(ApiError::InvalidInput);
        }
        let one: One<RawProject> = self
            .get(
                Url::parse(&format!("{ORIGIN}/v1/mods/{id}"))
                    .map_err(|_| ApiError::InvalidInput)?,
            )
            .await?;
        if one.data.id != id {
            return Err(ApiError::InvalidResponse);
        }
        project(one.data)
    }
    /// Required dependencies are selected only from the official version/loader
    /// filtered endpoint. Ambiguous incompatible requirements fail closed.
    pub async fn resolve_install(
        &self,
        project: u32,
        id: u32,
        version: &str,
        loader: &str,
    ) -> Result<InstallPlan, ApiError> {
        self.resolve_pinned_files(&[(project, id)], version, loader)
            .await
    }
    pub async fn resolve_pinned_files(
        &self,
        pins: &[(u32, u32)],
        version: &str,
        loader: &str,
    ) -> Result<InstallPlan, ApiError> {
        let (project, id) = *pins.first().ok_or(ApiError::InvalidInput)?;
        let plan = self.resolve_pack_files(pins, version, Some(loader)).await?;
        let mods = plan
            .files
            .into_iter()
            .map(|item| {
                if item.destination != PackFileDestination::Mods {
                    return Err(ApiError::UnsupportedClass);
                }
                Ok(ResolvedMod {
                    project_id: item.project_id,
                    version_id: item.file_id,
                    name: item.name,
                    version_number: item.version_number,
                    file: item.file,
                    required_by: item.required_by,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(InstallPlan {
            graph: ModrinthPlan {
                project_id: project.to_string(),
                version_id: id.to_string(),
                mods,
            },
            restricted: plan.restricted,
            incompatible: plan.incompatible,
        })
    }
    pub async fn resolve_pack_files(
        &self,
        pins: &[(u32, u32)],
        version: &str,
        loader: Option<&str>,
    ) -> Result<PackInstallPlan, ApiError> {
        if let Some(loader) = loader {
            loader_id(loader)?;
        }
        if pins.is_empty() || pins.len() > 256 || pins.iter().any(|(p, f)| *p == 0 || *f == 0) {
            return Err(ApiError::InvalidInput);
        }
        let mut queue: VecDeque<_> = pins.iter().map(|(p, f)| (*p, Some(*f), None)).collect();
        let mut selected = BTreeMap::new();
        let mut incompatible = BTreeSet::new();
        let mut restricted = Vec::new();
        while let Some((project, pinned, required_by)) = queue.pop_front() {
            if let Some(previous) = selected.get(&project) {
                let previous: &ResolvedPackFile = previous;
                if pinned.is_some_and(|id| previous.file_id != id.to_string()) {
                    return Err(ApiError::InvalidResponse);
                }
                continue;
            }
            if selected.len() >= 256 || queue.len() > 1024 || incompatible.contains(&project) {
                return Err(ApiError::InvalidResponse);
            }
            let metadata = self.project(project).await?;
            let destination = match metadata.class_id {
                6 => PackFileDestination::Mods,
                12 => PackFileDestination::ResourcePack,
                6552 => PackFileDestination::ShaderPack,
                _ => return Err(ApiError::UnsupportedClass),
            };
            let file_loader = if destination == PackFileDestination::Mods {
                Some(loader.ok_or(ApiError::InvalidInput)?)
            } else {
                None
            };
            let file = if let Some(id) = pinned {
                self.exact_file(project, id).await?
            } else {
                let page: Page<RawFile> = self
                    .get(files_url(project, Some(version), file_loader, 0)?)
                    .await?;
                validate_page(&page, 0)?;
                page.data
                    .into_iter()
                    .find(|file| file.is_available)
                    .ok_or(ApiError::NotFound)?
            };
            let expected_loader = match file_loader {
                Some("forge") => Some("Forge"),
                Some("fabric") => Some("Fabric"),
                Some("quilt") => Some("Quilt"),
                Some("neoforge") => Some("NeoForge"),
                None => None,
                _ => return Err(ApiError::InvalidInput),
            };
            let invalid = |field| ApiError::FileValidation {
                project_id: project,
                file_id: file.id,
                field,
            };
            if file.mod_id != project || file.game_id != MINECRAFT {
                return Err(invalid("identity"));
            }
            if !file.game_versions.iter().any(|v| v == version) {
                return Err(invalid("minecraft_version"));
            }
            if expected_loader.is_some_and(|loader| {
                !file
                    .game_versions
                    .iter()
                    .any(|v| v.eq_ignore_ascii_case(loader))
            }) {
                return Err(invalid("loader"));
            }
            let sha1 = file
                .hashes
                .iter()
                .find(|h| h.algo == 1)
                .filter(|h| h.value.len() == 40 && h.value.bytes().all(|b| b.is_ascii_hexdigit()))
                .ok_or_else(|| invalid("sha1"))?
                .value
                .to_ascii_lowercase();
            if file.file_length == 0
                || file.file_length > 512 * 1024 * 1024
                || file.file_name.len() > 240
                || file.file_name.contains(['/', '\\', ':'])
                || file.file_name.starts_with('.')
                || !file.file_name.to_ascii_lowercase().ends_with(
                    if destination == PackFileDestination::Mods {
                        ".jar"
                    } else {
                        ".zip"
                    },
                )
            {
                return Err(invalid("file_name_or_size"));
            }
            let (url, missing) = artifact_route(&metadata, &file)?;
            if let Some(missing) = missing {
                restricted.push(missing);
            }
            for dependency in &file.dependencies {
                match dependency.relation_type {
                    1 | 3 => {
                        queue.push_back((dependency.mod_id, None, Some(metadata.title.clone())))
                    }
                    5 => {
                        if selected.contains_key(&dependency.mod_id) {
                            return Err(ApiError::InvalidResponse);
                        }
                        incompatible.insert(dependency.mod_id);
                    }
                    2 | 4 | 6 => (),
                    _ => return Err(ApiError::InvalidResponse),
                }
            }
            selected.insert(
                project,
                ResolvedPackFile {
                    destination,
                    project_id: project.to_string(),
                    file_id: file.id.to_string(),
                    name: metadata.title,
                    version_number: file.display_name,
                    file: ModrinthFile {
                        url,
                        filename: file.file_name,
                        size: file.file_length,
                        sha1,
                        sha512: None,
                    },
                    required_by,
                },
            );
        }
        Ok(PackInstallPlan {
            files: selected.into_values().collect(),
            restricted,
            incompatible: incompatible.into_iter().map(|id| id.to_string()).collect(),
        })
    }
    pub async fn find_update(
        &self,
        project: u32,
        current: u32,
        version: &str,
        loader: &str,
    ) -> Result<Option<File>, ApiError> {
        let current = self.exact_file(project, current).await?;
        let current_date = date_key(&current.file_date)?;
        let page = self.files(project, Some(version), Some(loader), 0).await?;
        let mut newer = Vec::new();
        for file in page.files {
            let date = date_key(&file.date_published)?;
            if file.download_availability != DownloadAvailability::Unavailable
                && date > current_date
            {
                newer.push((date, file));
            }
        }
        Ok(newer
            .into_iter()
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, file)| file))
    }
}

fn date_key(value: &str) -> Result<(String, String), ApiError> {
    let value = value.strip_suffix('Z').ok_or(ApiError::InvalidResponse)?;
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    if seconds.len() != 19
        || seconds.bytes().enumerate().any(|(i, b)| match i {
            4 | 7 => b != b'-',
            10 => b != b'T',
            13 | 16 => b != b':',
            _ => !b.is_ascii_digit(),
        })
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(ApiError::InvalidResponse);
    }
    Ok((seconds.into(), format!("{fraction:0<9}")))
}

/// Availability controls automatic network acquisition, not whether the user
/// may supply already owned bytes with the exact official identity/hash/size.
fn artifact_route(
    project: &Project,
    file: &RawFile,
) -> Result<(String, Option<RestrictedFile>), ApiError> {
    let reason = if !file.is_available || !project.available {
        Some("unavailable")
    } else if project.allow_distribution == Some(false) || file.download_url.is_none() {
        Some("manual")
    } else {
        None
    };
    if let Some(reason) = reason {
        return Ok((
            String::new(),
            Some(RestrictedFile {
                project_id: file.mod_id.to_string(),
                file_id: file.id.to_string(),
                name: project.title.clone(),
                file_name: file.file_name.clone(),
                size_bytes: file.file_length,
                website_url: project.website_url.clone(),
                reason: reason.into(),
            }),
        ));
    }
    let url = file
        .download_url
        .as_deref()
        .filter(|url| official_file_url(url))
        .ok_or(ApiError::FileValidation {
            project_id: file.mod_id,
            file_id: file.id,
            field: "download_origin",
        })?;
    Ok((url.into(), None))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archived_or_distribution_restricted_exact_files_keep_a_verified_local_route() {
        let mut project = Project {
            icon_url: None,
            preview_url: None,
            gallery: vec![],
            class_id: 6,
            project_id: "11".into(),
            slug: "fixture".into(),
            title: "Fixture".into(),
            description: String::new(),
            author: String::new(),
            downloads: 0,
            website_url: Some("https://www.curseforge.com/minecraft/mc-mods/fixture".into()),
            allow_distribution: Some(true),
            available: true,
        };
        let mut file:RawFile=serde_json::from_value(serde_json::json!({"id":22,"gameId":432,"modId":11,"isAvailable":false,"fileStatus":8,"displayName":"Pinned fixture","fileName":"pinned.jar","releaseType":1,"fileDate":"2026-10-22T00:00:00Z","fileLength":7,"downloadUrl":"https://edge.forgecdn.net/files/1/pinned.jar","gameVersions":["1.21.1","Fabric"],"dependencies":[],"hashes":[{"algo":1,"value":"a".repeat(40)}]})).unwrap();
        let (url, missing) = artifact_route(&project, &file).unwrap();
        assert!(url.is_empty());
        let missing = missing.unwrap();
        assert_eq!(
            (
                missing.project_id.as_str(),
                missing.file_id.as_str(),
                missing.reason.as_str(),
                missing.size_bytes
            ),
            ("11", "22", "unavailable", 7)
        );
        assert_eq!(file.hashes[0].value, "a".repeat(40));
        file.is_available = true;
        project.allow_distribution = Some(false);
        let (url, missing) = artifact_route(&project, &file).unwrap();
        assert!(url.is_empty());
        assert_eq!(missing.unwrap().reason, "manual");
        project.allow_distribution = Some(true);
        file.download_url = None;
        assert_eq!(
            artifact_route(&project, &file).unwrap().1.unwrap().reason,
            "manual"
        );
        file.download_url = Some("https://edge.forgecdn.net/files/1/pinned.jar".into());
        let (url, missing) = artifact_route(&project, &file).unwrap();
        assert!(!url.is_empty());
        assert!(missing.is_none());
    }
    #[test]
    fn update_dates_and_official_artifact_origins_are_bounded() {
        assert!(
            date_key("2026-10-22T12:30:45Z").unwrap() < date_key("2026-10-22T12:30:45.1Z").unwrap()
        );
        assert_eq!(
            date_key("2026-10-22T12:30:45.10Z").unwrap(),
            date_key("2026-10-22T12:30:45.1Z").unwrap()
        );
        assert!(date_key("2026-10-22T12:30:45+08:00").is_err());
        assert!(official_file_url(
            "https://edge.forgecdn.net/files/123/456/mod.jar"
        ));
        for url in [
            "https://edge.forgecdn.net.evil.invalid/files/1/a.jar",
            "https://edge.forgecdn.net/files/1/a.jar?api_key=secret",
            "https://user@edge.forgecdn.net/files/1/a.jar",
            "https://edge.forgecdn.net:444/files/1/a.jar",
            "http://edge.forgecdn.net/files/1/a.jar",
            "https://mediafilez.forgecdn.net/other/a.jar",
        ] {
            assert!(!official_file_url(url));
        }
    }
}

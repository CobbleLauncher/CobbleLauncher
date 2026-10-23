// SPDX-License-Identifier: MIT
//! Native provider credentials. No key crosses IPC, enters logs, or is saved in app preferences.
use cobble_content::curseforge::{ContentKind, CurseForgeClient, FilePage, SearchPage};
use serde::Serialize;

fn checked_key(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty() && value.len() <= 4096 && value.bytes().all(|byte| byte.is_ascii_graphic()))
        .then(|| value.to_owned())
}
pub(crate) fn credential() -> Option<String> {
    // Explicit opt-in only. Normal release builds do not reference the compile-time key.
    #[cfg(feature = "embedded-curseforge-key")]
    let embedded = option_env!("COURSE_FORGE_API");
    #[cfg(not(feature = "embedded-curseforge-key"))]
    let embedded = None;
    select_credential(std::env::var("COURSE_FORGE_API").ok(), embedded)
}
fn select_credential(runtime: Option<String>, embedded: Option<&str>) -> Option<String> {
    runtime
        .and_then(checked_key)
        .or_else(|| embedded.and_then(|value| checked_key(value.to_owned())))
}
pub(crate) fn client() -> Result<CurseForgeClient, String> {
    CurseForgeClient::new(credential().ok_or("CurseForge 服务尚未配置")?)
        .map_err(|error| error.to_string())
}
#[derive(Serialize)]
pub struct ProviderAvailability {
    available: bool,
    reason: Option<&'static str>,
}

#[tauri::command]
pub fn get_curseforge_availability() -> ProviderAvailability {
    let available = credential().is_some();
    ProviderAvailability {
        available,
        reason: (!available).then_some("CurseForge 服务尚未配置"),
    }
}
#[tauri::command]
pub async fn search_curseforge(
    query: String,
    kind: ContentKind,
    minecraft_version: Option<String>,
    loader: Option<String>,
    offset: u32,
) -> Result<SearchPage, String> {
    client()?
        .search(
            &query,
            kind,
            minecraft_version.as_deref(),
            loader.as_deref(),
            offset,
        )
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub async fn list_curseforge_files(
    project_id: String,
    minecraft_version: Option<String>,
    loader: Option<String>,
    offset: u32,
) -> Result<FilePage, String> {
    let project_id = project_id
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("CurseForge 项目编号无效")?;
    client()?
        .files(
            project_id,
            minecraft_version.as_deref(),
            loader.as_deref(),
            offset,
        )
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_and_malformed_credentials_are_not_available() {
        assert!(checked_key(String::new()).is_none());
        assert!(checked_key("value\r\nheader".into()).is_none());
        assert!(checked_key("a".repeat(4097)).is_none());
        assert!(checked_key("synthetic-key".into()).is_some());
        assert_eq!(
            select_credential(Some("runtime-synthetic".into()), Some("embedded-synthetic")),
            Some("runtime-synthetic".into())
        );
        assert_eq!(
            select_credential(None, Some("embedded-synthetic")),
            Some("embedded-synthetic".into())
        );
        assert_eq!(
            select_credential(Some("invalid\nvalue".into()), Some("embedded-synthetic")),
            Some("embedded-synthetic".into())
        );
        assert_eq!(select_credential(None, None), None);
    }
    #[test]
    #[cfg(debug_assertions)]
    #[ignore = "Requires explicit runtime development credential; official metadata only, no cache or downloads"]
    fn official_live_search_and_files_without_persisting_metadata() {
        let client = client().unwrap_or_else(|error| panic!("provider unavailable: {error}"));
        tauri::async_runtime::block_on(async {
            let page = client
                .search("jei", ContentKind::Mod, None, None, 0)
                .await
                .unwrap_or_else(|error| panic!("official search failed: {error}"));
            assert!(
                !page.hits.is_empty(),
                "official search returned no matching projects"
            );
            let project_id = page.hits[0].project_id.parse().unwrap();
            let files = client
                .files(project_id, None, None, 0)
                .await
                .unwrap_or_else(|error| panic!("official files failed: {error}"));
            assert!(
                !files.files.is_empty(),
                "official project returned no files"
            );
        });
    }
}

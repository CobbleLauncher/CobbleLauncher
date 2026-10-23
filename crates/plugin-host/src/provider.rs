// SPDX-License-Identifier: MIT
//! Pure provider planning/normalization in an owned, bounded Wasm worker.
use cobble_plugin_api::PluginError;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

pub const MAX_METADATA_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperation {
    Search,
    Detail,
    Download,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SearchQuery {
    pub java_release: String,
    pub text: String,
    pub format: Option<String>,
    pub mod_namespace: Option<String>,
    pub cursor: Option<String>,
    pub limit: u8,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResourceKey {
    pub id: String,
    pub version: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequest {
    pub operation: ProviderOperation,
    pub search: Option<SearchQuery>,
    pub key: Option<ResourceKey>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GetRequest {
    pub url: String,
    pub maximum_bytes: u32,
}
#[derive(Clone, Debug)]
pub struct GetResponse {
    pub status: u16,
    pub body: Vec<u8>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderResource {
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: u32,
    pub format: String,
    pub author: String,
    pub attribution: String,
    pub license: Option<String>,
    pub java_release: Option<String>,
    pub namespaces: Vec<String>,
    pub preview_url: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileProposal {
    pub url: String,
    pub suggested_filename: String,
    pub original_filename: String,
    pub declared_bytes: u64,
    pub format: String,
    pub id: String,
    pub version: u32,
    pub title: String,
    pub author: String,
    pub attribution: String,
    pub license: Option<String>,
    pub sha256: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderOutput {
    pub items: Vec<ProviderResource>,
    pub next_cursor: Option<String>,
    pub detail: Option<ProviderResource>,
    pub download: Option<FileProposal>,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "stage", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Job {
    Plan {
        request: ProviderRequest,
    },
    Normalize {
        request: ProviderRequest,
        statuses: Vec<u16>,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub(crate) enum Reply {
    Ready,
    Plan(Vec<GetRequest>),
    Normalized(Box<ProviderOutput>),
}

fn invalid(message: &str) -> PluginError {
    PluginError::new("PLUGIN_INVALID_PROVIDER", message)
}
fn text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}
fn id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
impl ProviderRequest {
    pub fn validate(&self) -> Result<(), PluginError> {
        match self.operation {
            ProviderOperation::Search => {
                let q = self
                    .search
                    .as_ref()
                    .ok_or_else(|| invalid("Missing search query"))?;
                if self.key.is_some()
                    || !text(&q.java_release, 32)
                    || !text(&q.text, 800)
                    || !(1..=50).contains(&q.limit)
                    || q.format
                        .as_ref()
                        .is_some_and(|v| !matches!(v.as_str(), "schem" | "litematic"))
                    || q.mod_namespace
                        .as_ref()
                        .is_some_and(|v| v.is_empty() || !text(v, 64))
                    || q.cursor
                        .as_ref()
                        .is_some_and(|v| v.is_empty() || !text(v, 8192))
                {
                    return Err(invalid("Invalid search query"));
                }
            }
            _ => {
                let key = self
                    .key
                    .as_ref()
                    .ok_or_else(|| invalid("Missing resource key"))?;
                if self.search.is_some() || !id(&key.id) || key.version == 0 {
                    return Err(invalid("Invalid resource key"));
                }
            }
        }
        Ok(())
    }
}
pub fn validate_url(value: &str, approved_origins: &[String]) -> Result<(), PluginError> {
    if approved_origins.is_empty() || approved_origins.len() > 4 || value.len() > 32768 {
        return Err(invalid("Provider origin is not approved"));
    }
    let url = url::Url::parse(value).map_err(|_| invalid("Invalid provider URL"))?;
    if url.scheme() != "https"
        || url.domain().is_none()
        || url.port().is_some_and(|p| p != 443)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !approved_origins
            .iter()
            .any(|origin| origin == &url.origin().ascii_serialization())
    {
        return Err(invalid("Provider URL exceeds its origin permission"));
    }
    Ok(())
}
fn validate_plan(plan: &[GetRequest], origins: &[String]) -> Result<(), PluginError> {
    if plan.is_empty() || plan.len() > 2 {
        return Err(invalid("Provider plan must contain one or two GETs"));
    }
    for request in plan {
        validate_url(&request.url, origins)?;
        if request.maximum_bytes == 0 || request.maximum_bytes as usize > MAX_METADATA_BYTES {
            return Err(invalid("Provider metadata budget exceeded"));
        }
    }
    Ok(())
}
fn validate_resource(resource: &ProviderResource, origins: &[String]) -> Result<(), PluginError> {
    if !id(&resource.id)
        || resource.version == 0
        || resource.title.is_empty()
        || !text(&resource.title, 4096)
        || resource.description.len() > 16384
        || resource
            .description
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        || !text(&resource.author, 1024)
        || !text(&resource.format, 64)
        || !text(&resource.attribution, 64)
        || resource.license.as_ref().is_some_and(|v| !text(v, 256))
        || resource.namespaces.len() > 1024
        || resource.namespaces.iter().any(|s| !text(s, 128))
        || resource.java_release.as_ref().is_some_and(|s| !text(s, 64))
    {
        return Err(invalid("Invalid normalized resource"));
    }
    if let Some(url) = &resource.preview_url {
        validate_url(url, origins)?;
    }
    Ok(())
}
fn validate_output(
    output: &ProviderOutput,
    request: &ProviderRequest,
    origins: &[String],
) -> Result<(), PluginError> {
    if output.items.len() > 50 || output.next_cursor.as_ref().is_some_and(|v| !text(v, 8192)) {
        return Err(invalid("Provider output exceeds budget"));
    }
    for resource in &output.items {
        validate_resource(resource, origins)?;
    }
    if let Some(resource) = &output.detail {
        validate_resource(resource, origins)?;
    }
    match request.operation {
        ProviderOperation::Search if output.detail.is_some() || output.download.is_some() => {
            return Err(invalid("Unexpected provider output"));
        }
        ProviderOperation::Detail
            if output.detail.is_none() || !output.items.is_empty() || output.download.is_some() =>
        {
            return Err(invalid("Unexpected provider output"));
        }
        ProviderOperation::Download if output.download.is_none() || !output.items.is_empty() => {
            return Err(invalid("Missing download proposal"));
        }
        _ => {}
    }
    if let Some(key) = &request.key
        && (output
            .detail
            .as_ref()
            .is_some_and(|resource| resource.id != key.id || resource.version != key.version)
            || output
                .download
                .as_ref()
                .is_some_and(|file| file.id != key.id || file.version != key.version))
    {
        return Err(invalid("Provider changed the selected resource"));
    }
    if let Some(file) = &output.download {
        validate_url(&file.url, origins)?;
        if !id(&file.id)
            || file.version == 0
            || file.declared_bytes == 0
            || file.declared_bytes > cobble_plugin_api::MAX_FILE_BYTES as u64
            || !matches!(file.format.as_str(), "schem" | "litematic")
            || file.suggested_filename.starts_with('.')
            || !id(&file.suggested_filename)
            || !file
                .suggested_filename
                .ends_with(&format!(".{}", file.format))
            || !text(&file.original_filename, 1024)
            || file.original_filename.contains(['/', '\\', ':'])
            || !text(&file.title, 4096)
            || !text(&file.author, 1024)
            || !text(&file.attribution, 64)
            || file.license.as_ref().is_some_and(|v| !text(v, 256))
            || file
                .sha256
                .as_ref()
                .is_some_and(|v| v.len() != 64 || !v.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid("Invalid normalized download proposal"));
        }
    }
    Ok(())
}
/// Convenience one-operation wrapper; use ProviderSession to reuse compilation.
pub fn plan_provider(
    host: &Path,
    component: &[u8],
    request: &ProviderRequest,
    approved_origins: &[String],
    cancel: Arc<AtomicBool>,
) -> Result<Vec<GetRequest>, PluginError> {
    ProviderSession::open(host, component, cancel.clone())?.plan(request, approved_origins, cancel)
}
/// Convenience one-operation wrapper; use ProviderSession for plan+HTTP+normalize.
pub fn normalize_provider(
    host: &Path,
    component: &[u8],
    request: &ProviderRequest,
    responses: &[GetResponse],
    approved_origins: &[String],
    cancel: Arc<AtomicBool>,
) -> Result<ProviderOutput, PluginError> {
    ProviderSession::open(host, component, cancel.clone())?.normalize(
        request,
        responses,
        approved_origins,
        cancel,
    )
}
mod session;
pub use session::ProviderSession;
#[cfg(test)]
mod tests {
    use super::*;
    fn origins() -> Vec<String> {
        vec!["https://www.mcschematic.top".into()]
    }
    #[test]
    fn plan_enforces_approved_origin_and_budget() {
        let valid = GetRequest {
            url: "https://www.mcschematic.top/api/v1/blueprints".into(),
            maximum_bytes: 100,
        };
        assert!(validate_plan(std::slice::from_ref(&valid), &origins()).is_ok());
        for url in [
            "http://www.mcschematic.top/api/v1/blueprints",
            "https://user:secret@www.mcschematic.top/api/v1/blueprints",
            "https://127.0.0.1/api",
            "https://other.example/api",
            "https://www.mcschematic.top:444/api",
            "https://www.mcschematic.top/api#fragment",
        ] {
            assert!(
                validate_plan(
                    &[GetRequest {
                        url: url.into(),
                        ..valid.clone()
                    }],
                    &origins()
                )
                .is_err()
            );
        }
        assert!(validate_plan(std::slice::from_ref(&valid), &[]).is_err());
        assert!(
            validate_plan(
                &[GetRequest {
                    maximum_bytes: MAX_METADATA_BYTES as u32 + 1,
                    ..valid.clone()
                }],
                &origins()
            )
            .is_err()
        );
        assert!(validate_plan(&[valid.clone(), valid.clone(), valid], &origins()).is_err());
    }
    #[test]
    fn resource_license_is_optional_bounded_metadata() {
        let mut resource = ProviderResource {
            id: "opaque-key".into(),
            title: "Fixture".into(),
            description: String::new(),
            version: 1,
            format: "schem".into(),
            author: "Author".into(),
            attribution: "original".into(),
            license: None,
            java_release: None,
            namespaces: vec![],
            preview_url: None,
        };
        assert!(validate_resource(&resource, &origins()).is_ok());
        resource.license = Some("MIT".into());
        assert!(validate_resource(&resource, &origins()).is_ok());
        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["license"], "MIT");
        for invalid_license in ["x".repeat(257), "MIT\nforged".into()] {
            resource.license = Some(invalid_license);
            assert!(validate_resource(&resource, &origins()).is_err());
        }
    }
    #[test]
    fn proposals_bind_selected_identity_and_cannot_choose_paths() {
        let request = ProviderRequest {
            operation: ProviderOperation::Download,
            search: None,
            key: Some(ResourceKey {
                id: "chosen".into(),
                version: 1,
            }),
        };
        let file = FileProposal {
            url: "https://www.mcschematic.top/file".into(),
            suggested_filename: "chosen.schem".into(),
            original_filename: "original.schem".into(),
            declared_bytes: 1,
            format: "schem".into(),
            id: "chosen".into(),
            version: 1,
            title: "Original fixture".into(),
            author: "Fixture author".into(),
            attribution: "unknown".into(),
            license: None,
            sha256: None,
        };
        let mut output = ProviderOutput {
            items: vec![],
            next_cursor: None,
            detail: None,
            download: Some(file.clone()),
        };
        assert!(validate_output(&output, &request, &origins()).is_ok());
        for bad in [
            FileProposal {
                id: "other".into(),
                ..file.clone()
            },
            FileProposal {
                suggested_filename: "../world.schem".into(),
                ..file.clone()
            },
            FileProposal {
                url: "https://other.example/file".into(),
                ..file.clone()
            },
            FileProposal {
                declared_bytes: cobble_plugin_api::MAX_FILE_BYTES as u64 + 1,
                ..file.clone()
            },
        ] {
            output.download = Some(bad);
            assert!(validate_output(&output, &request, &origins()).is_err());
        }
    }
}

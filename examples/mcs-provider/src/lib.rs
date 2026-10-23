// SPDX-License-Identifier: MIT
//! Original import-free provider. Native code authorizes and performs all HTTP.
wit_bindgen::generate!({path:"../../contracts/provider",world:"blueprint-provider"});
use cobble_blueprint_catalog as catalog;
use exports::launcher::blueprint_provider::provider as wire;

fn failure(code: &str, message: &str) -> wire::Failure {
    wire::Failure {
        code: code.into(),
        message: message.into(),
    }
}
fn metadata(error: catalog::CatalogError) -> wire::Failure {
    if error == catalog::CatalogError::UnsupportedGameVersion {
        return failure(
            "PROVIDER_UNSUPPORTED_GAME_VERSION",
            "This source does not support filtering by that game version",
        );
    }
    failure("PROVIDER_METADATA", &format!("{error:?}"))
}

fn plan(request: &wire::Request) -> Result<Vec<wire::GetRequest>, wire::Failure> {
    let urls = match request.operation {
        wire::Operation::Search => {
            let search = request
                .search
                .as_ref()
                .filter(|_| request.key.is_none())
                .ok_or_else(|| {
                    failure(
                        "PROVIDER_INVALID_REQUEST",
                        "Search requires search fields only",
                    )
                })?;
            let format = match search.format.as_deref() {
                None => None,
                Some("schem") => Some(catalog::ViewerFormat::Schem),
                Some("litematic") => Some(catalog::ViewerFormat::Litematic),
                _ => {
                    return Err(failure(
                        "PROVIDER_UNSUPPORTED_FORMAT",
                        "Unsupported blueprint format",
                    ));
                }
            };
            let query = catalog::SearchQuery {
                java_release: &search.java_release,
                text: &search.text,
                format,
                mod_namespace: search.mod_namespace.as_deref(),
                cursor: search.cursor.as_deref(),
                limit: search.limit,
            };
            vec![query.url().map_err(metadata)?.to_string()]
        }
        wire::Operation::Detail | wire::Operation::Download => {
            let key = request
                .key
                .as_ref()
                .filter(|_| request.search.is_none())
                .ok_or_else(|| {
                    failure(
                        "PROVIDER_INVALID_REQUEST",
                        "Detail/download requires a resource key only",
                    )
                })?;
            if !valid_uuid(&key.id) || key.version == 0 || key.version > i32::MAX as u32 {
                return Err(failure(
                    "PROVIDER_INVALID_REQUEST",
                    "Invalid blueprint ID or version",
                ));
            }
            vec![
                format!("{}/api/v1/blueprints/{}", catalog::MCS_ORIGIN, key.id),
                format!(
                    "{}/api/v1/blueprints/{}/versions/{}",
                    catalog::MCS_ORIGIN,
                    key.id,
                    key.version
                ),
            ]
        }
    };
    Ok(urls
        .into_iter()
        .map(|url| wire::GetRequest {
            url,
            maximum_bytes: catalog::MAX_API_BYTES as u32,
        })
        .collect())
}

fn normalize(
    request: &wire::Request,
    responses: &[wire::GetResponse],
) -> Result<wire::Output, wire::Failure> {
    let requests = plan(request)?;
    if requests.len() != responses.len() {
        return Err(failure(
            "PROVIDER_RESPONSE_COUNT",
            "Responses must match the approved GET plan",
        ));
    }
    for (request, response) in requests.iter().zip(responses) {
        if response.status != 200 {
            return Err(failure(
                "PROVIDER_HTTP",
                "Public blueprint API did not return HTTP 200",
            ));
        }
        if response.body.len() > request.maximum_bytes as usize {
            return Err(failure(
                "PROVIDER_LIMIT",
                "Blueprint response exceeds its budget",
            ));
        }
    }
    let mut output = wire::Output {
        items: vec![],
        next_cursor: None,
        detail: None,
        download: None,
    };
    match request.operation {
        wire::Operation::Search => {
            let page = catalog::parse_page(&responses[0].body).map_err(metadata)?;
            output.next_cursor = page.next_cursor;
            output.items = page
                .items
                .into_iter()
                .map(|item| wire::Resource {
                    id: item.id,
                    title: item.title,
                    description: item.description,
                    version: item.current_version.number,
                    format: source_format(&item.current_version.source_format),
                    author: item.author.display_name,
                    attribution: attribution(&item.attribution),
                    license: None,
                    java_release: item.source_game_version.and_then(|version| version.release),
                    namespaces: item.namespaces,
                    preview_url: preview_url(item.preview_url),
                })
                .collect();
        }
        wire::Operation::Detail => {
            let detail: catalog::BlueprintDetail = serde_json::from_slice(&responses[0].body)
                .map_err(|_| metadata(catalog::CatalogError::InvalidJson))?;
            let version: catalog::BlueprintVersion = serde_json::from_slice(&responses[1].body)
                .map_err(|_| metadata(catalog::CatalogError::InvalidJson))?;
            let key = request.key.as_ref().expect("request checked by plan");
            if detail.id != key.id
                || version.blueprint_id != key.id
                || version.number != key.version
            {
                return Err(failure(
                    "PROVIDER_IDENTITY",
                    "Response does not match the requested blueprint/version",
                ));
            }
            if !detail.anonymously_readable || detail.visibility != "public" {
                return Err(metadata(catalog::CatalogError::RestrictedResource));
            }
            output.detail = Some(wire::Resource {
                id: detail.id,
                title: detail.title,
                description: String::new(),
                version: version.number,
                format: source_format(&version.source_format),
                author: detail.author.display_name,
                attribution: attribution(&detail.attribution),
                license: None,
                java_release: None,
                namespaces: vec![],
                preview_url: None,
            });
        }
        wire::Operation::Download => {
            let proposal = catalog::original_file_proposal(&responses[0].body, &responses[1].body)
                .map_err(metadata)?;
            let key = request.key.as_ref().expect("request checked by plan");
            if proposal.provenance.blueprint_id != key.id
                || proposal.provenance.version != key.version
            {
                return Err(failure(
                    "PROVIDER_IDENTITY",
                    "Response does not match the requested blueprint/version",
                ));
            }
            let format = match proposal.format {
                catalog::ViewerFormat::Schem => "schem",
                catalog::ViewerFormat::Litematic => "litematic",
            }
            .to_owned();
            output.download = Some(wire::FileProposal {
                url: proposal.request_url,
                suggested_filename: proposal.suggested_filename,
                original_filename: proposal.original_filename,
                declared_bytes: proposal.declared_byte_size,
                format,
                id: proposal.provenance.blueprint_id,
                version: proposal.provenance.version,
                title: proposal.provenance.title,
                author: proposal.provenance.author.display_name,
                attribution: attribution(&proposal.provenance.attribution),
                license: proposal.provenance.license,
                sha256: proposal.provenance.sha256,
            });
        }
    }
    Ok(output)
}
fn source_format(format: &catalog::SourceFormat) -> String {
    match format {
        catalog::SourceFormat::WorldEdit => "schem",
        catalog::SourceFormat::Litematica => "litematic",
        catalog::SourceFormat::Create => "create",
        catalog::SourceFormat::BuildingGadgets => "building_gadgets",
        catalog::SourceFormat::BedrockEdition => "bedrock_edition",
        catalog::SourceFormat::Mcedit => "mcedit",
        catalog::SourceFormat::Unknown => "unknown",
    }
    .into()
}
fn attribution(value: &catalog::Attribution) -> String {
    match value {
        catalog::Attribution::Original => "original",
        catalog::Attribution::Repost => "repost",
        catalog::Attribution::Unknown => "unknown",
    }
    .into()
}
fn preview_url(value: Option<String>) -> Option<String> {
    let origin = url::Url::parse(catalog::MCS_ORIGIN).ok()?;
    let url = origin.join(&value?).ok()?;
    (url.origin() == origin.origin()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then(|| url.to_string())
}
fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}
struct McsProvider;
impl wire::Guest for McsProvider {
    fn plan(request: wire::Request) -> Result<Vec<wire::GetRequest>, wire::Failure> {
        plan(&request)
    }
    fn normalize(
        request: wire::Request,
        responses: Vec<wire::GetResponse>,
    ) -> Result<wire::Output, wire::Failure> {
        normalize(&request, &responses)
    }
}
export!(McsProvider);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_game_filter_is_distinct_and_empty_filter_is_explicitly_unfiltered() {
        let mut request = search();
        let query = request.search.as_mut().unwrap();
        query.java_release = "26.3".into();
        query.text.clear();
        query.format = None;
        assert_eq!(
            plan(&request).unwrap_err().code,
            "PROVIDER_UNSUPPORTED_GAME_VERSION"
        );
        request.search.as_mut().unwrap().java_release.clear();
        let requests = plan(&request).unwrap();
        let url = url::Url::parse(&requests[0].url).unwrap();
        assert!(!url.query_pairs().any(|(name, _)| name == "gameVersion"));
    }
    fn search() -> wire::Request {
        wire::Request {
            operation: wire::Operation::Search,
            key: None,
            search: Some(wire::SearchQuery {
                java_release: "1.21.1".into(),
                text: "小屋 & q=other".into(),
                format: Some("schem".into()),
                mod_namespace: None,
                cursor: None,
                limit: 20,
            }),
        }
    }
    fn key(operation: wire::Operation) -> wire::Request {
        wire::Request {
            operation,
            search: None,
            key: Some(wire::ResourceKey {
                id: "11111111-1111-4111-8111-111111111111".into(),
                version: 1,
            }),
        }
    }
    fn response(body: &[u8]) -> wire::GetResponse {
        wire::GetResponse {
            status: 200,
            body: body.to_vec(),
        }
    }
    #[test]
    fn plans_anonymous_encoded_search_and_normalizes_real_shape() {
        let request = search();
        let plan = plan(&request).unwrap();
        assert_eq!(plan.len(), 1);
        assert!(
            plan[0]
                .url
                .starts_with("https://www.mcschematic.top/api/v1/blueprints?")
        );
        assert!(plan[0].url.contains("gameVersion=java-1.21"));
        assert!(plan[0].url.contains("%26+q%3Dother"));
        let output = normalize(
            &request,
            &[response(include_bytes!("../fixtures/page.json"))],
        )
        .unwrap();
        assert_eq!(output.items.len(), 2);
        assert_eq!(output.items[0].format, "schem");
        assert_eq!(output.next_cursor.as_deref(), Some("opaque-next-cursor"));
    }
    #[test]
    fn download_uses_original_file_and_preserves_unknown_license_and_digest() {
        let output = normalize(
            &key(wire::Operation::Download),
            &[
                response(include_bytes!("../fixtures/detail.json")),
                response(include_bytes!("../fixtures/version.json")),
            ],
        )
        .unwrap();
        let proposal = output.download.unwrap();
        assert_eq!(proposal.format, "schem");
        assert!(proposal.url.ends_with("/versions/1/download"));
        assert!(!proposal.url.ends_with("/source"));
        assert!(proposal.license.is_none() && proposal.sha256.is_none());
    }
    #[test]
    fn rejects_missing_responses_http_errors_and_mismatched_resource() {
        assert!(normalize(&search(), &[]).is_err());
        assert!(
            normalize(
                &search(),
                &[wire::GetResponse {
                    status: 403,
                    body: vec![]
                }]
            )
            .is_err()
        );
        let mut wrong = key(wire::Operation::Download);
        wrong.key.as_mut().unwrap().version = 2;
        assert!(
            normalize(
                &wrong,
                &[
                    response(include_bytes!("../fixtures/detail.json")),
                    response(include_bytes!("../fixtures/version.json"))
                ]
            )
            .is_err()
        );
        wrong.key.as_mut().unwrap().id = "../../private".into();
        assert!(plan(&wrong).is_err());
    }
    #[test]
    fn unsupported_download_formats_still_have_public_detail_metadata() {
        let mut version: serde_json::Value =
            serde_json::from_slice(include_bytes!("../fixtures/version.json")).unwrap();
        version["sourceFormat"] = serde_json::json!("create");
        let responses = [
            response(include_bytes!("../fixtures/detail.json")),
            response(&serde_json::to_vec(&version).unwrap()),
        ];
        let output = normalize(&key(wire::Operation::Detail), &responses).unwrap();
        let detail = output.detail.unwrap();
        assert_eq!(detail.format, "create");
        assert!(
            detail.license.is_none(),
            "attribution must not invent a license"
        );
        assert!(output.download.is_none());
        assert!(normalize(&key(wire::Operation::Download), &responses).is_err());
    }
}

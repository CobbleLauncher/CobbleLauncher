// SPDX-License-Identifier: MIT
use super::*;
use serde_json::{Value, json};

const PAGE: &[u8] = include_bytes!("../tests/fixtures/page.json");
const DETAIL: &[u8] = include_bytes!("../tests/fixtures/detail.json");
const VERSION: &[u8] = include_bytes!("../tests/fixtures/version.json");

#[test]
fn maps_documented_java_families_without_claiming_patch_compatibility() {
    for (release, family) in [
        ("1.7.10", "java-1.7.10"),
        ("1.12.2", "java-1.12"),
        ("1.20.1", "java-1.20"),
        ("1.21", "java-1.21"),
        ("1.21.11", "java-1.21"),
    ] {
        assert_eq!(java_version_family(release).unwrap(), family);
    }
    for release in [
        "1.8.9",
        "1.7",
        "1.22",
        "26.1",
        "24w14a",
        "1.21-pre1",
        "1.021",
        "1.21.",
        "1.21.1.1",
    ] {
        assert_eq!(
            java_version_family(release),
            Err(CatalogError::UnsupportedGameVersion)
        );
    }
}

#[test]
fn search_encodes_literal_filters_and_opaque_cursor_on_the_public_endpoint() {
    let query = SearchQuery {
        java_release: "1.21.1",
        text: "小屋 & q=other",
        format: Some(ViewerFormat::Litematic),
        mod_namespace: Some("create"),
        cursor: Some("opaque+/=cursor"),
        limit: 20,
    };
    let url = query.url().unwrap();
    assert_eq!(url.origin().ascii_serialization(), MCS_ORIGIN);
    assert_eq!(url.path(), "/api/v1/blueprints");
    let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(pairs["gameVersion"], "java-1.21");
    assert_eq!(pairs["format"], "litematica");
    assert_eq!(pairs["q"], "小屋 & q=other");
    assert_eq!(pairs["mod"], "create");
    assert_eq!(pairs["cursor"], "opaque+/=cursor");
    assert_eq!(pairs["extra"], "none");
    assert_eq!(pairs.len(), 7);
    assert_eq!(
        SearchQuery { limit: 51, ..query }.url(),
        Err(CatalogError::InvalidQuery)
    );
    assert_eq!(
        SearchQuery {
            mod_namespace: Some("create&extra=private"),
            ..query
        }
        .url(),
        Err(CatalogError::InvalidQuery)
    );
}

#[test]
fn unfiltered_search_is_explicit_and_does_not_guess_another_family() {
    let url = SearchQuery {
        java_release: "",
        text: "",
        format: Some(ViewerFormat::Schem),
        mod_namespace: None,
        cursor: None,
        limit: 20,
    }
    .url()
    .unwrap();
    let pairs: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert!(!pairs.contains_key("gameVersion"));
    assert_eq!(pairs["format"], "world_edit");
    assert_eq!(pairs["limit"], "20");
    assert_eq!(
        java_version_family("26.3"),
        Err(CatalogError::UnsupportedGameVersion)
    );
}

#[test]
fn retains_actual_source_release_and_unknown_formats_without_inventing_support() {
    let page = parse_page(PAGE).unwrap();
    assert_eq!(page.total_count, 2);
    assert_eq!(page.next_cursor.as_deref(), Some("opaque-next-cursor"));
    assert_eq!(
        page.items[0]
            .source_game_version
            .as_ref()
            .unwrap()
            .release
            .as_deref(),
        Some("1.21.11")
    );
    assert_eq!(
        page.items[0].current_version.source_format.viewer_format(),
        Some(ViewerFormat::Schem)
    );
    assert_eq!(
        page.items[1].current_version.source_format.viewer_format(),
        None
    );
    assert_eq!(
        parse_page(&vec![b' '; MAX_API_BYTES + 1]).unwrap_err(),
        CatalogError::InputTooLarge
    );
    let mut broken: Value = serde_json::from_slice(PAGE).unwrap();
    broken["items"][0]["id"] = json!("../not-an-id");
    assert_eq!(
        parse_page(&serde_json::to_vec(&broken).unwrap()).unwrap_err(),
        CatalogError::InvalidMetadata
    );
}

#[test]
fn proposes_original_bytes_with_attribution_and_explicit_unknown_license_and_digest() {
    let proposal = original_file_proposal(DETAIL, VERSION).unwrap();
    assert_eq!(proposal.format, ViewerFormat::Schem);
    assert!(proposal.request_url.ends_with("/versions/1/download"));
    assert_eq!(proposal.original_filename, "盆栽.schem");
    assert_eq!(
        proposal.suggested_filename,
        "11111111-1111-4111-8111-111111111111-v1.schem"
    );
    assert_eq!(proposal.declared_byte_size, 2237);
    assert_eq!(proposal.provenance.attribution, Attribution::Repost);
    assert_eq!(proposal.provenance.author.display_name, "Synthetic Author");
    assert_eq!(proposal.provenance.license, None);
    assert_eq!(proposal.provenance.sha256, None);
    // Fixture deliberately advertises a different viewerSourceFormat and size.
    // These must never silently replace the original download.
    assert!(!proposal.request_url.ends_with("/source"));
}

#[test]
fn refuses_restricted_cross_origin_cross_item_unsupported_and_path_like_proposals() {
    let mut detail: Value = serde_json::from_slice(DETAIL).unwrap();
    detail["anonymouslyReadable"] = json!(false);
    assert_eq!(
        original_file_proposal(&serde_json::to_vec(&detail).unwrap(), VERSION).unwrap_err(),
        CatalogError::RestrictedResource
    );
    for (key, value, expected) in [
        (
            "downloadUrl",
            json!("https://other.example/file.schem"),
            CatalogError::InvalidMetadata,
        ),
        (
            "downloadUrl",
            json!(
                "https://user:secret@www.mcschematic.top/api/v1/blueprints/11111111-1111-4111-8111-111111111111/versions/1/download"
            ),
            CatalogError::InvalidMetadata,
        ),
        (
            "downloadUrl",
            json!("/api/v1/blueprints/11111111-1111-4111-8111-111111111111/versions/1/source"),
            CatalogError::InvalidMetadata,
        ),
        (
            "blueprintId",
            json!("33333333-3333-4333-8333-333333333333"),
            CatalogError::InvalidMetadata,
        ),
        (
            "originalFilename",
            json!("../../world.schem"),
            CatalogError::InvalidMetadata,
        ),
        (
            "originalFilename",
            json!("world.litematic"),
            CatalogError::UnsupportedFormat,
        ),
        (
            "sourceFormat",
            json!("create"),
            CatalogError::UnsupportedFormat,
        ),
        (
            "validationState",
            json!("client_unverified"),
            CatalogError::InvalidMetadata,
        ),
    ] {
        let mut version: Value = serde_json::from_slice(VERSION).unwrap();
        version[key] = value;
        assert_eq!(
            original_file_proposal(DETAIL, &serde_json::to_vec(&version).unwrap()).unwrap_err(),
            expected,
            "{key}"
        );
    }
}

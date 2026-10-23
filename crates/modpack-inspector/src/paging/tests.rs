// SPDX-License-Identifier: MIT
use super::*;
use crate::{OverrideFile, PackSnapshot};
use serde_json::json;

fn snapshot() -> PackSnapshot {
    // Original metadata only; no external pack, fixture or source copied.
    PackSnapshot {
        index: json!({"formatVersion":1,"game":"minecraft","name":"Synthetic pack","versionId":"1","dependencies":{"minecraft":"1.21.1","future-loader":"unknown"},"files":[]}),
        overrides: vec![],
    }
}
fn query(section: InspectionSection) -> InspectionQuery {
    InspectionQuery {
        section,
        ..Default::default()
    }
}

#[test]
fn all_fifty_thousand_entries_are_searchable_and_last_page_is_real() {
    let mut current = snapshot();
    current.overrides = (0..50_000)
        .rev()
        .map(|i| OverrideFile {
            path: format!("overrides/config/item-{i:05}.json"),
            size: i,
            crc32: i as u32,
        })
        .collect();
    let model = PreparedInspection::new(InspectionRequest {
        current,
        baseline: None,
    })
    .unwrap();
    let first = model.query(&query(InspectionSection::Current)).unwrap();
    assert_eq!(first.total_rows, 50_000);
    assert_eq!(first.matched_rows, 50_000);
    assert_eq!(first.rows.len(), 50);
    assert_eq!(first.next_offset, Some(50));
    assert!(first.rows[0].path.text.ends_with("item-00000.json"));
    let mut last_query = query(InspectionSection::Current);
    last_query.offset = 49_950;
    let last = model.query(&last_query).unwrap();
    assert_eq!(last.rows.len(), 50);
    assert!(last.rows[49].path.text.ends_with("item-49999.json"));
    assert_eq!(last.next_offset, None);
    let mut search = query(InspectionSection::Current);
    search.search = "ITEM-49999".into();
    let found = model.query(&search).unwrap();
    assert_eq!(found.total_rows, 50_000);
    assert_eq!(found.matched_rows, 1);
    assert_eq!(found.rows[0].id, last.rows[49].id);
    // Search includes CRC metadata beyond the displayed path and all unpaged records.
    search.search = "\"crc32\":49998".into();
    assert_eq!(model.query(&search).unwrap().matched_rows, 1);
    assert_eq!(model.summary().current.override_files, 50_000);
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PreparedInspection>();
}

#[test]
fn overview_dependency_setting_filters_before_counts_search_and_pagination() {
    let mut current = snapshot();
    current.index["dependencies"] = json!({
        "minecraft": "1.21.1",
        "future-loader": "unknown"
    });
    let model = PreparedInspection::new(InspectionRequest {
        current,
        baseline: None,
    })
    .unwrap();

    let default_page = model.query(&query(InspectionSection::Overview)).unwrap();
    assert_eq!(default_page.total_rows, 9);
    assert_eq!(default_page.matched_rows, 9);
    assert!(
        default_page
            .rows
            .iter()
            .any(|row| row.source == RowSource::Dependency)
    );

    let mut filtered = query(InspectionSection::Overview);
    filtered.show_dependencies = false;
    filtered.limit = 2;
    let first = model.query(&filtered).unwrap();
    assert_eq!(first.total_rows, 7);
    assert_eq!(first.matched_rows, 7);
    assert_eq!(first.next_offset, Some(2));
    assert!(
        first
            .rows
            .iter()
            .all(|row| row.source != RowSource::Dependency)
    );

    filtered.offset = 2;
    let next = model.query(&filtered).unwrap();
    assert_eq!(next.query.offset, 2);
    assert_eq!(next.rows.len(), 2);
    assert!(
        next.rows
            .iter()
            .all(|row| row.source != RowSource::Dependency)
    );

    filtered.offset = 0;
    filtered.search = "dependencies".into();
    let search = model.query(&filtered).unwrap();
    assert_eq!(search.total_rows, 7);
    assert_eq!(search.matched_rows, 0);
    assert!(search.rows.is_empty());

    let old_query: InspectionQuery = serde_json::from_value(json!({
        "section":"overview", "search":"", "offset":0, "limit":50
    }))
    .unwrap();
    assert!(old_query.show_dependencies);
}

#[test]
fn diff_categories_and_baseline_reference_original_complete_metadata() {
    let mut baseline = snapshot();
    baseline.overrides = vec![
        OverrideFile {
            path: "overrides/a".into(),
            size: 1,
            crc32: 1,
        },
        OverrideFile {
            path: "overrides/b".into(),
            size: 1,
            crc32: 1,
        },
        OverrideFile {
            path: "overrides/gone".into(),
            size: 1,
            crc32: 1,
        },
    ];
    let mut current = snapshot();
    current.overrides = vec![
        OverrideFile {
            path: "overrides/a".into(),
            size: 2,
            crc32: 2,
        },
        OverrideFile {
            path: "overrides/b".into(),
            size: 1,
            crc32: 1,
        },
        OverrideFile {
            path: "overrides/new".into(),
            size: 1,
            crc32: 1,
        },
    ];
    let model = PreparedInspection::new(InspectionRequest {
        current,
        baseline: Some(baseline),
    })
    .unwrap();
    assert_eq!(
        model.summary().changes,
        Some(ChangeCounts {
            added: 1,
            removed: 1,
            changed: 1,
            unchanged: 1
        })
    );
    let page = model.query(&query(InspectionSection::Changes)).unwrap();
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row.change.unwrap())
            .collect::<Vec<_>>(),
        [
            ChangeKind::Changed,
            ChangeKind::Unchanged,
            ChangeKind::Removed,
            ChangeKind::Added
        ]
    );
    assert!(page.rows[2].before.is_some());
    assert!(page.rows[2].after.is_none());
    let old = model.query(&query(InspectionSection::Baseline)).unwrap();
    assert_eq!(old.rows.len(), 3);
    assert!(
        old.rows
            .iter()
            .all(|row| row.before.is_some() && row.after.is_none())
    );
    // Compare rows refer to the same original inventory records; no owned file delta.
    assert!(
        model
            .changes
            .iter()
            .all(|index| matches!(model.rows[*index].path, RowPath::File(_)))
    );
}

#[test]
fn long_unicode_metadata_is_exactly_reconstructible_without_shortened_cells() {
    let long = format!("{}TAIL_SENTINEL", "世界🙂".repeat(3000));
    let mut current = snapshot();
    current.index["summary"] = json!(long);
    current.index["files"] = json!([{"path":"mods/a.jar","fileSize":1,"hashes":{"sha512":"a"},"downloads":[],"customField":long}]);
    let model = PreparedInspection::new(InspectionRequest {
        current,
        baseline: None,
    })
    .unwrap();
    let mut search = query(InspectionSection::Overview);
    search.search = "current/summary".into();
    let row = model.query(&search).unwrap().rows.remove(0);
    let preview = row.after.unwrap();
    assert!(!preview.complete);
    assert!(preview.text.len() <= MAX_PREVIEW_BYTES);
    search.search = "tail_sentinel".into();
    assert_eq!(model.query(&search).unwrap().matched_rows, 1);
    let expected = serde_json::to_string(&long).unwrap();
    let mut text = String::new();
    let mut offset = 0;
    loop {
        let chunk = model
            .detail(&InspectionDetailQuery {
                row_id: row.id.clone(),
                field: DetailField::After,
                offset,
                limit: 1024,
            })
            .unwrap();
        assert!(chunk.text.len() <= 1024);
        assert_eq!(chunk.total_bytes as usize, expected.len());
        text.push_str(&chunk.text);
        match chunk.next_offset {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => break,
        }
    }
    assert_eq!(text, expected);
    let file = model
        .query(&query(InspectionSection::Current))
        .unwrap()
        .rows
        .remove(0);
    assert!(!file.after.as_ref().unwrap().complete);
    // Unknown per-file fields remain available, not just typed install fields.
    let detail = model
        .detail(&InspectionDetailQuery {
            row_id: file.id,
            field: DetailField::After,
            ..Default::default()
        })
        .unwrap();
    assert!(detail.text.contains("customField"));
}

#[test]
fn query_ranges_and_protocol_are_explicitly_validated() {
    let mut current = snapshot();
    current.index["summary"] = json!("世");
    let model = PreparedInspection::new(InspectionRequest {
        current,
        baseline: None,
    })
    .unwrap();
    let mut request = query(InspectionSection::Overview);
    request.limit = 101;
    assert_eq!(model.query(&request), Err(QueryError::InvalidQuery));
    request.limit = 50;
    request.offset = u32::MAX;
    assert_eq!(model.query(&request), Err(QueryError::InvalidRange));
    request.offset = 0;
    request.search = "x".repeat(1025);
    assert_eq!(model.query(&request), Err(QueryError::InvalidQuery));
    request.search = "current/summary".into();
    let row = model.query(&request).unwrap().rows.remove(0);
    assert_eq!(
        model.detail(&InspectionDetailQuery {
            row_id: row.id.clone(),
            field: DetailField::Before,
            ..Default::default()
        }),
        Err(QueryError::UnavailableField)
    );
    // JSON string starts with a quote followed by the 3-byte Chinese character.
    assert_eq!(
        model.detail(&InspectionDetailQuery {
            row_id: row.id.clone(),
            field: DetailField::After,
            offset: 2,
            limit: 1024
        }),
        Err(QueryError::InvalidRange)
    );
    let page = model.query(&query(InspectionSection::Overview)).unwrap();
    let mut envelope = InspectionEnvelope::page(page);
    assert!(envelope.validate_protocol().is_ok());
    let json = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json["kind"], "page");
    assert_eq!(json["protocol"], INSPECTION_PAGE_PROTOCOL);
    let decoded: InspectionEnvelope = serde_json::from_value(json).unwrap();
    assert_eq!(decoded, envelope);
    let mut malformed = decoded;
    if let InspectionContent::Page { page } = &mut malformed.content {
        page.next_offset = Some(999);
    }
    assert_eq!(malformed.validate_protocol(), Err(QueryError::InvalidQuery));
    envelope.protocol = "unknown@2".into();
    assert_eq!(envelope.validate_protocol(), Err(QueryError::InvalidQuery));
}

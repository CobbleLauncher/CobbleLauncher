// SPDX-License-Identifier: MIT
use super::*;
fn doc(base: &str, local: &str, new: &str) -> JsonMergeDocument {
    preview_json_merge(
        "config/test.json",
        base.as_bytes(),
        local.as_bytes(),
        new.as_bytes(),
    )
    .unwrap()
}
fn choice(pointer: &str, side: JsonMergeSide) -> JsonFieldChoice {
    JsonFieldChoice {
        pointer: pointer.into(),
        side,
    }
}
#[test]
fn json_three_way_preserves_local_additions_and_requires_deletion_array_conflicts() {
    let d = doc(
        r#"{"a":1,"deleted":1,"array":[1],"unchanged":true}"#,
        r#"{"a":2,"array":[2],"localOnly":7,"unchanged":true}"#,
        r#"{"a":3,"deleted":2,"array":[3],"upstreamOnly":8,"unchanged":true}"#,
    );
    assert_eq!(d.preview.fields.iter().filter(|f| f.conflict).count(), 3);
    assert_eq!(
        merge_json_document(&d, &[]).unwrap_err(),
        "PACK_UPDATE_JSON_CHOICE_REQUIRED"
    );
    let merged = merge_json_document(
        &d,
        &[
            choice("/a", JsonMergeSide::Local),
            choice("/deleted", JsonMergeSide::Local),
            choice("/array", JsonMergeSide::Upstream),
        ],
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&merged).unwrap(),
        serde_json::json!({"a":2,"array":[3],"localOnly":7,"upstreamOnly":8,"unchanged":true})
    );
}
#[test]
fn json_three_way_structural_changes_have_no_overlapping_choices() {
    let d = doc(
        r#"{"nested":{"a":1},"type":{"x":1}}"#,
        r#"{"type":{"x":2}}"#,
        r#"{"nested":{"a":2},"type":[1,2]}"#,
    );
    assert_eq!(
        d.preview
            .fields
            .iter()
            .map(|f| f.pointer.as_str())
            .collect::<Vec<_>>(),
        vec!["/nested", "/type"]
    );
    assert!(d.preview.fields.iter().all(|f| f.conflict));
    let merged = merge_json_document(
        &d,
        &[
            choice("/nested", JsonMergeSide::Local),
            choice("/type", JsonMergeSide::Upstream),
        ],
    )
    .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&merged).unwrap(),
        serde_json::json!({"type":[1,2]})
    );
}
#[test]
fn json_three_way_escapes_pointers_and_distinguishes_null_from_absence() {
    let d = doc(
        r#"{"a/b":{"~key":null}}"#,
        r#"{"a/b":{}}"#,
        r#"{"a/b":{"~key":null,"new":2}}"#,
    );
    assert!(
        d.preview
            .fields
            .iter()
            .any(|f| f.pointer == "/a~1b/~0key" && !f.local.exists)
    );
    let merged = merge_json_document(&d, &[]).unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&merged).unwrap(),
        serde_json::json!({"a/b":{"new":2}})
    );
    assert!(merge_json_document(&d, &[choice("/foreign", JsonMergeSide::Local)]).is_err());
}
#[test]
fn json_three_way_unsupported_and_ambiguous_inputs_fall_back_whole_file() {
    for raw in [
        br#"{"duplicate":1,"duplicate":2}"#.as_slice(),
        b"{ // comment\n\"a\":1}",
        b"[1,2]",
        b"",
        b"not JSON",
    ] {
        assert!(preview_json_merge("config/a.json", raw, b"{}", b"{}").is_none());
    }
    assert!(preview_json_merge("config/a.jsonc", b"{}", b"{}", b"{}").is_none());
    assert!(
        preview_json_merge(
            "config/a.json",
            &vec![b' '; MAX_JSON_MERGE_BYTES + 1],
            b"{}",
            b"{}"
        )
        .is_none()
    );
}

// SPDX-License-Identifier: MIT
use super::*;
use serde_json::json;

fn manifest() -> PluginManifest {
    super::super::tests::manifest()
}

#[test]
fn yaml_roundtrip_is_scoped_and_preview_does_not_write() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = manifest();
    write_value(directory.path(), &manifest, "page_size", json!(20), None).unwrap();
    write_value(
        directory.path(),
        &manifest,
        "enabled",
        json!(false),
        Some("game-a"),
    )
    .unwrap();
    let before = fs::read(directory.path().join("fixture.json")).unwrap();
    let stored = load(directory.path(), "fixture").unwrap();
    let yaml = export(&manifest, &stored, None).unwrap();
    assert_eq!(
        preview(&manifest, &Stored::default(), None, &yaml)
            .unwrap()
            .values,
        BTreeMap::from([("page_size".into(), json!(20))])
    );
    assert!(preview(&manifest, &stored, Some("game-a"), &yaml).is_err());
    let yaml = export(&manifest, &stored, Some("game-a")).unwrap();
    assert_eq!(
        preview(&manifest, &Stored::default(), Some("game-b"), &yaml)
            .unwrap()
            .values,
        BTreeMap::from([("enabled".into(), json!(false))])
    );
    assert_eq!(
        fs::read(directory.path().join("fixture.json")).unwrap(),
        before
    );
}

fn document(values: &str) -> String {
    format!("schema_version: 1\nplugin_id: fixture\nscope: global\nvalues:\n{values}\n")
}

#[test]
fn invalid_yaml_and_schema_fail_without_accepting_partial_values() {
    let manifest = manifest();
    for yaml in [
        document("  page_size: '20'"),
        document("  page_size: 99"),
        document("  unknown: true"),
        document("  enabled: false"),
        document("  page_size: 20\n  page_size: 50"),
        document("  page_size: !custom 20"),
        document("  page_size: !include /etc/passwd"),
        document("  page_size: &value 20"),
        document("  page_size: *missing"),
        document("  <<: {page_size: 20}"),
        document("  page_size: [20]"),
        document("  page_size: .nan"),
        document("  page_size: 20\nextra: true"),
        document("  page_size: 20").replace("fixture", "another-plugin"),
        document("  page_size: 20").replace("schema_version: 1", "schema_version: 2"),
        format!(
            "{}---\n{}",
            document("  page_size: 20"),
            document("  page_size: 50")
        ),
        "a".repeat(MAX_YAML_BYTES + 1),
    ] {
        assert!(preview(&manifest, &Stored::default(), None, &yaml).is_err());
    }
}

#[test]
fn yaml_strings_preserve_unicode_and_ambiguous_scalar_text() {
    let mut manifest = manifest();
    let field = &mut manifest
        .contributions
        .as_mut()
        .unwrap()
        .settings
        .as_mut()
        .unwrap()
        .categories[0]
        .fields[0];
    field.kind = SettingKind::String;
    field.options.clear();
    field.default = json!("true: 20 中文 # comment");
    field.max_length = Some(128);
    let yaml = export(&manifest, &Stored::default(), None).unwrap();
    let preview = preview(&manifest, &Stored::default(), None, &yaml).unwrap();
    assert_eq!(
        preview.values["page_size"],
        json!("true: 20 中文 # comment")
    );
}

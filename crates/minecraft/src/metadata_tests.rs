// SPDX-License-Identifier: MIT
//! Original synthetic metadata, not copied from a launcher implementation.
use super::*;
use serde_json::{Value, json};

fn fixture() -> Value {
    let artifact = |path: &str| json!({"path":path,"url":format!("https://example.invalid/{path}"),"sha1":"0123456789abcdef0123456789abcdef01234567","size":12});
    json!({
        "id":"fixture-modern", "mainClass":"fixture.Main", "javaVersion":{"majorVersion":21},
        "arguments":{"game":["--demo"],"jvm":["-Xmx1G"]},
        "assetIndex":{"id":"fixture-assets","url":"https://example.invalid/index.json","size":12,"sha1":"0123456789abcdef0123456789abcdef01234567"},
        "downloads":{"client":artifact("client.jar")},
        "logging":{"client":{"file":{"id":"fixture.xml","url":"https://example.invalid/log.xml","size":12,"sha1":"0123456789abcdef0123456789abcdef01234567"}}},
        "libraries":[
            {"downloads":{"artifact":artifact("fixture/common.jar")}},
            {"rules":[{"action":"allow","os":{"name":"linux"}}],"downloads":{"artifact":artifact("fixture/linux.jar")}},
            {"rules":[{"action":"allow"},{"action":"disallow","os":{"name":"windows"}},{"action":"allow","os":{"name":"windows","arch":"amd64","version":"^10\\."}}],
             "downloads":{"artifact":artifact("fixture/windows.jar"),"classifiers":{"natives-windows-64":artifact("fixture/native64.jar")}},"natives":{"windows":"natives-windows-${arch}"}},
            {"rules":[{"action":"allow","features":{"future_feature":false}}],"downloads":{"artifact":artifact("fixture/feature.jar")}}
        ]
    })
}
fn parse(value: Value) -> Result<VersionMetadata, MetadataError> {
    let bytes = serde_json::to_vec(&value).unwrap();
    let digest = format!("{:x}", Sha1::digest(&bytes));
    VersionMetadata::parse_verified(&bytes, "fixture-modern", &digest)
}
fn windows() -> PlatformContext {
    PlatformContext {
        os: "windows".into(),
        arch: "amd64".into(),
        os_version: Some("10.0.26100".into()),
        features: HashMap::new(),
    }
}

#[test]
fn modern_plan_selects_ordered_rules_native_logging_and_index() {
    let metadata = parse(fixture()).unwrap();
    assert!(!metadata.source_bytes().is_empty());
    let plan = metadata.download_plan(&windows()).unwrap();
    assert_eq!(plan.version_id, "fixture-modern");
    assert_eq!(plan.java_major, 21);
    assert_eq!(plan.main_class, "fixture.Main");
    let paths: Vec<_> = plan.files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "versions/fixture-modern/fixture-modern.jar",
            "libraries/fixture/common.jar",
            "libraries/fixture/windows.jar",
            "libraries/fixture/native64.jar",
            "libraries/fixture/feature.jar",
            "assets/log_configs/fixture.xml",
            "assets/indexes/fixture-assets.json"
        ]
    );
    assert_eq!(plan.files[3].kind, FileKind::Native);
    let mut platform = windows();
    platform.os_version = Some("11.0".into());
    assert!(
        !metadata
            .download_plan(&platform)
            .unwrap()
            .files
            .iter()
            .any(|file| file.kind == FileKind::Native)
    );
    platform.os = "linux".into();
    assert!(
        metadata
            .download_plan(&platform)
            .unwrap()
            .files
            .iter()
            .any(|file| file.path.as_str() == "libraries/fixture/linux.jar")
    );
}

#[test]
fn digest_identity_client_and_modern_metadata_are_required() {
    let bytes = serde_json::to_vec(&fixture()).unwrap();
    assert!(matches!(
        VersionMetadata::parse_verified(&bytes, "fixture-modern", &"0".repeat(40)),
        Err(MetadataError::DigestMismatch)
    ));
    assert!(matches!(
        VersionMetadata::parse_verified(&bytes, "other", &format!("{:x}", Sha1::digest(&bytes))),
        Err(MetadataError::InvalidMetadata)
    ));
    let mut value = fixture();
    value["downloads"] = json!({});
    assert!(matches!(parse(value), Err(MetadataError::InvalidMetadata)));
    let mut value = fixture();
    value["inheritsFrom"] = json!("parent");
    assert!(matches!(
        parse(value),
        Err(MetadataError::Unsupported("inherited profiles"))
    ));
    let mut value = fixture();
    value.as_object_mut().unwrap().remove("arguments");
    assert!(matches!(
        parse(value),
        Err(MetadataError::Unsupported("legacy arguments"))
    ));
}

#[test]
fn unknown_rules_bad_regex_missing_platform_and_unsafe_paths_fail_explicitly() {
    let mut value = fixture();
    value["libraries"][0]["rules"] = json!([{"action":"allow","future_constraint":true}]);
    assert!(matches!(parse(value), Err(MetadataError::InvalidMetadata)));
    let mut value = fixture();
    value["libraries"][2]["rules"][2]["os"]["version"] = json!("[");
    assert!(matches!(
        parse(value).unwrap().download_plan(&windows()),
        Err(MetadataError::Unsupported("OS version regex"))
    ));
    let mut platform = windows();
    platform.os_version = None;
    assert!(matches!(
        parse(fixture()).unwrap().download_plan(&platform),
        Err(MetadataError::Unsupported("OS version required by rule"))
    ));
    let mut value = fixture();
    value["libraries"][0]["downloads"]["artifact"]["path"] = json!("../escape.jar");
    assert!(matches!(
        parse(value).unwrap().download_plan(&windows()),
        Err(MetadataError::InvalidMetadata)
    ));
    let mut value = fixture();
    value["libraries"][0]["downloads"]["artifact"]["sha1"] = json!("bad");
    assert!(matches!(
        parse(value).unwrap().download_plan(&windows()),
        Err(MetadataError::InvalidMetadata)
    ));
}

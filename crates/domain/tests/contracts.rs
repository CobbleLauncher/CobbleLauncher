// SPDX-License-Identifier: MIT
use cobble_domain::{
    contracts::{ContractError, ContractModel, parse_contract},
    generated::*,
};
use serde::Serialize;
use serde_json::{Value, json};

const LIMIT: usize = 2 * 1024 * 1024;

fn check<T: ContractModel + Serialize>(source: &str) {
    let original: Value = serde_json::from_str(source).unwrap();
    let model: T = parse_contract(source, LIMIT).unwrap();
    assert_eq!(serde_json::to_value(model).unwrap(), original);
    let mut unexpected = original.clone();
    unexpected["access_token"] = json!("must never be accepted or echoed");
    assert!(matches!(
        parse_contract::<T>(&unexpected.to_string(), LIMIT),
        Err(ContractError::InvalidSchema)
    ));
    for key in original.as_object().unwrap().keys() {
        let mut missing = original.clone();
        missing.as_object_mut().unwrap().remove(key);
        assert!(
            parse_contract::<T>(&missing.to_string(), LIMIT).is_err(),
            "missing required field: {key}"
        );
    }
}

#[test]
fn all_canonical_models_roundtrip_and_reject_unknown_or_missing_fields() {
    check::<InstanceSpec>(include_str!("../../../contracts/examples/instance.json"));
    check::<InstanceLock>(include_str!("../../../contracts/examples/lock.json"));
    check::<InstallPlan>(include_str!("../../../contracts/examples/plan.json"));
    check::<PluginManifest>(include_str!("../../../contracts/examples/plugin.json"));
    check::<TaskEvent>(include_str!("../../../contracts/examples/event.json"));
}

#[test]
fn nested_unknown_fields_enums_hashes_paths_and_numbers_are_validated() {
    let base: Value =
        serde_json::from_str(include_str!("../../../contracts/examples/plan.json")).unwrap();
    let cases = [
        (
            "/artifacts/0/sources/0/authorization",
            json!("invented_grant"),
        ),
        (
            "/artifacts/0/sources/0/url",
            json!("http://example.invalid/file"),
        ),
        ("/artifacts/0/expected_hashes/sha256", json!("bad")),
        ("/artifacts/0/size", json!(9_007_199_254_740_992_u64)),
        ("/artifacts/0/size", json!(-1)),
        ("/artifacts/0/size", json!(1.5)),
        ("/operations/0/path", json!("../worlds/secret")),
        ("/schema_version", json!(2)),
    ];
    for (pointer, replacement) in cases {
        let mut invalid = base.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            parse_contract::<InstallPlan>(&invalid.to_string(), LIMIT).is_err(),
            "accepted {pointer}"
        );
    }
    let mut invalid = base;
    invalid["artifacts"][0]["sources"][0]["token"] = json!("private");
    assert!(parse_contract::<InstallPlan>(&invalid.to_string(), LIMIT).is_err());
}

#[test]
fn null_and_absent_are_distinct_and_optional_null_is_rejected() {
    let mut source: Value =
        serde_json::from_str(include_str!("../../../contracts/examples/instance.json")).unwrap();
    assert!(parse_contract::<InstanceSpec>(&source.to_string(), LIMIT).is_ok());
    source.as_object_mut().unwrap().remove("account_id");
    assert!(parse_contract::<InstanceSpec>(&source.to_string(), LIMIT).is_err());
    let mut plan: Value =
        serde_json::from_str(include_str!("../../../contracts/examples/plan.json")).unwrap();
    plan["artifacts"][0]["sources"][0]["project_id"] = Value::Null;
    assert!(parse_contract::<InstallPlan>(&plan.to_string(), LIMIT).is_err());
}

#[test]
fn malformed_and_oversize_input_errors_never_echo_input() {
    assert!(matches!(
        parse_contract::<InstanceSpec>("{\"token\":\"private\"", LIMIT),
        Err(ContractError::InvalidJson)
    ));
    assert!(matches!(
        parse_contract::<InstanceSpec>("private", 2),
        Err(ContractError::InputTooLarge)
    ));
}

#[test]
fn integral_json_float_preserves_schema_integer_semantics() {
    let mut source: Value =
        serde_json::from_str(include_str!("../../../contracts/examples/instance.json")).unwrap();
    source["memory_mib"] = json!(512.0);
    assert_eq!(
        parse_contract::<InstanceSpec>(&source.to_string(), LIMIT)
            .unwrap()
            .memory_mib,
        512
    );
}

// SPDX-License-Identifier: MIT
use super::*;
use crate::{PermissionNode, PluginManifest};
use serde_json::json;

fn manifest() -> PluginManifest {
    serde_json::from_value(json!({
        "id":"mesh-fixture", "name":"Mesh fixture", "publisher":"Fixture", "version":"0.1.0",
        "api_version":"0.2.0", "capabilities":["report"], "file_extensions":[], "component_sha256":"a".repeat(64),
        "frontend":{"api_version":"0.1.0","entry":"ui/index.html","assets_sha256":"b".repeat(64)},
        "permissions":[{"permission":"launcher:ui/webview"},{"permission":"launcher:mesh/events/send","namespaces":["org.example.coordinates"]}]
    })).unwrap()
}

#[test]
fn namespace_and_event_names_are_exact_and_reserved_lifecycle_names_are_rejected() {
    assert!(valid_mesh_namespace("org.example.coordinates-v1"));
    assert!(valid_mesh_namespace(&format!(
        "{}.{}",
        "a".repeat(63),
        "b".repeat(32)
    )));
    for value in [
        "org",
        "cobble.internal",
        "org.*",
        "org..example",
        "Org.example",
        "org.example-",
        "org.1example",
        "org.example/path",
        "org.示例",
        "org.example\n",
    ] {
        assert!(!valid_mesh_namespace(value), "{value}");
    }
    assert!(!valid_mesh_namespace(&format!("{}.org", "a".repeat(64))));
    assert!(!valid_mesh_namespace(&format!(
        "{}.{}",
        "a".repeat(63),
        "b".repeat(33)
    )));
    assert!(valid_mesh_event("waypoint.updated:v1"));
    assert!(valid_mesh_event(&"a".repeat(64)));
    for value in [
        "",
        "*",
        "point Updated",
        "Updated",
        "point\n",
        "event/other",
        "newListener",
        "removeListener",
    ] {
        assert!(!valid_mesh_event(value), "{value}");
    }
    for reserved in RESERVED_MESH_EVENTS {
        assert!(!valid_mesh_event(reserved));
    }
    assert!(!valid_mesh_event(&"a".repeat(65)));
}

#[test]
fn payload_counts_encoded_utf8_and_bounds_container_depth() {
    validate_mesh_payload(&json!(null)).unwrap();
    validate_mesh_payload(&json!("a".repeat(1022))).unwrap();
    assert!(validate_mesh_payload(&json!("a".repeat(1023))).is_err());
    validate_mesh_payload(&json!("中".repeat(340))).unwrap();
    assert!(validate_mesh_payload(&json!("中".repeat(341))).is_err());
    assert!(validate_mesh_payload(&json!("\0".repeat(171))).is_err());
    let mut nested = json!(null);
    for _ in 0..8 {
        nested = json!([nested]);
    }
    validate_mesh_payload(&nested).unwrap();
    assert!(validate_mesh_payload(&json!([nested])).is_err());
}

#[test]
fn mesh_permissions_require_frontend_exact_namespaces_and_one_node_per_direction() {
    let current = manifest();
    current.validate().unwrap();
    let mut invalid = current.clone();
    invalid.frontend = None;
    assert!(invalid.validate().is_err());
    for namespaces in [
        vec![],
        vec!["cobble.internal".into()],
        vec!["org.example".into(); 2],
        (0..9).map(|index| format!("org.ns{index}")).collect(),
    ] {
        invalid = current.clone();
        invalid.permissions.as_mut().unwrap()[1] = PermissionNode::MeshEventsSend { namespaces };
        assert!(invalid.validate().is_err());
    }
    invalid = current.clone();
    invalid
        .permissions
        .as_mut()
        .unwrap()
        .push(PermissionNode::MeshEventsSend {
            namespaces: vec!["org.other".into()],
        });
    assert!(invalid.validate().is_err());
    invalid = current;
    invalid
        .permissions
        .as_mut()
        .unwrap()
        .push(PermissionNode::MeshEventsReceive {
            namespaces: vec!["org.example.coordinates".into()],
        });
    invalid.validate().unwrap();
    assert!(serde_json::from_value::<PermissionNode>(json!({"permission":"launcher:mesh/events/send","namespaces":["org.example"],"peer_id":"spoof"})).is_err());
}

#[test]
fn namespace_expansion_and_receive_direction_require_review_but_reordering_does_not() {
    let mut previous = manifest();
    previous.permissions.as_mut().unwrap()[1] = PermissionNode::MeshEventsSend {
        namespaces: vec!["org.b".into(), "org.a".into()],
    };
    let mut next = previous.clone();
    next.permissions.as_mut().unwrap()[1] = PermissionNode::MeshEventsSend {
        namespaces: vec!["org.a".into(), "org.b".into()],
    };
    assert!(!next.requires_permission_approval(&previous));
    next.permissions.as_mut().unwrap()[1] = PermissionNode::MeshEventsSend {
        namespaces: vec!["org.a".into(), "org.b".into(), "org.c".into()],
    };
    assert!(next.requires_permission_approval(&previous));
    next = previous.clone();
    next.permissions
        .as_mut()
        .unwrap()
        .push(PermissionNode::MeshEventsReceive {
            namespaces: vec!["org.a".into()],
        });
    assert!(next.requires_permission_approval(&previous));
    let composer: PluginManifest = serde_json::from_str(include_str!(
        "../../../../examples/session-card/plugin.json"
    ))
    .unwrap();
    assert!(
        !composer.effective_permissions().iter().any(|node| matches!(
            node,
            PermissionNode::MeshEventsSend { .. } | PermissionNode::MeshEventsReceive { .. }
        ))
    );
    let provider: PluginManifest = serde_json::from_str(include_str!(
        "../../../../examples/mcs-provider/plugin.json"
    ))
    .unwrap();
    assert!(
        !provider.effective_permissions().iter().any(|node| matches!(
            node,
            PermissionNode::MeshEventsSend { .. } | PermissionNode::MeshEventsReceive { .. }
        ))
    );
}

#[test]
fn total_node_budget_does_not_relax_existing_capability_or_scope_requirements() {
    let mut diagnostic: PluginManifest = serde_json::from_str(include_str!(
        "../../../../examples/crash-doctor/plugin.json"
    ))
    .unwrap();
    diagnostic.permissions.as_mut().unwrap().extend([
        PermissionNode::MeshEventsSend {
            namespaces: vec!["org.example".into()],
        },
        PermissionNode::MeshEventsReceive {
            namespaces: vec!["org.example".into()],
        },
    ]);
    assert_eq!(diagnostic.permissions.as_ref().unwrap().len(), 12);
    diagnostic.validate().unwrap();
    diagnostic
        .permissions
        .as_mut()
        .unwrap()
        .retain(|node| !matches!(node, PermissionNode::GameRecoveryRead {}));
    assert!(diagnostic.validate().is_err());
}

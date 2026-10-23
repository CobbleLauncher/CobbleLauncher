// SPDX-License-Identifier: MIT
use cobble_plugin_host::provider::*;
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
fn host() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cobble-plugin-host"))
}
fn cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}
#[test]
#[ignore = "requires built target/mcs-provider.component.wasm"]
fn actual_provider_component_plans_and_normalizes_original_fixtures() {
    let component = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/mcs-provider.component.wasm"),
    )
    .unwrap();
    let override_host = std::env::var_os("COBBLE_TEST_PROVIDER_HOST").map(std::path::PathBuf::from);
    let executable = override_host.as_deref().unwrap_or_else(|| host());
    let startup = std::time::Instant::now();
    let mut session = ProviderSession::open(executable, &component, cancel()).unwrap();
    let startup_ms = startup.elapsed().as_millis();
    let pid = session.process_id();
    let calls = std::time::Instant::now();
    let origins = vec!["https://www.mcschematic.top".into()];
    let request = ProviderRequest {
        operation: ProviderOperation::Search,
        search: Some(SearchQuery {
            java_release: "1.21.1".into(),
            text: "Synthetic".into(),
            format: Some("schem".into()),
            mod_namespace: None,
            cursor: None,
            limit: 20,
        }),
        key: None,
    };
    let first_cancel = cancel();
    let plan = session
        .plan(&request, &origins, first_cancel.clone())
        .unwrap();
    first_cancel.store(true, std::sync::atomic::Ordering::Release);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(plan.len(), 1);
    assert!(plan[0].url.contains("gameVersion=java-1.21"));
    let response = GetResponse {
        status: 200,
        body: include_bytes!("../../blueprint-catalog/tests/fixtures/page.json").to_vec(),
    };
    let page = session
        .normalize(&request, &[response], &origins, cancel())
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].title, "Synthetic Planter");
    assert_eq!(page.items[0].java_release.as_deref(), Some("1.21.11"));
    let key = ResourceKey {
        id: page.items[0].id.clone(),
        version: page.items[0].version,
    };
    let responses = vec![
        GetResponse {
            status: 200,
            body: include_bytes!("../../blueprint-catalog/tests/fixtures/detail.json").to_vec(),
        },
        GetResponse {
            status: 200,
            body: include_bytes!("../../blueprint-catalog/tests/fixtures/version.json").to_vec(),
        },
    ];
    for operation in [ProviderOperation::Detail, ProviderOperation::Download] {
        let request = ProviderRequest {
            operation: operation.clone(),
            search: None,
            key: Some(key.clone()),
        };
        let first_cancel = cancel();
        let plan = session
            .plan(&request, &origins, first_cancel.clone())
            .unwrap();
        first_cancel.store(true, std::sync::atomic::Ordering::Release);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(plan.len(), 2);
        let output = session
            .normalize(&request, &responses, &origins, cancel())
            .unwrap();
        if operation == ProviderOperation::Detail {
            assert_eq!(output.detail.unwrap().id, key.id);
        } else {
            let proposal = output.download.unwrap();
            assert_eq!(proposal.id, key.id);
            assert!(proposal.url.ends_with("/versions/1/download"));
            assert!(!proposal.url.contains("/source"));
            assert_eq!(proposal.original_filename, "盆栽.schem");
            assert_eq!(proposal.declared_bytes, 2237);
            assert!(proposal.license.is_none());
            assert!(proposal.sha256.is_none());
        }
    }
    assert_eq!(session.process_id(), pid);
    let calls_ms = calls.elapsed().as_millis();
    let cancelled = Arc::new(AtomicBool::new(true));
    assert_eq!(
        session
            .plan(&request, &origins, cancelled)
            .unwrap_err()
            .code,
        "CANCEL_REQUESTED"
    );
    session.close();
    assert!(session.has_exited().unwrap());
    println!(
        "provider retained: startup_ms={startup_ms}, six_calls_including_3x100ms_http_gaps_ms={calls_ms}, same owned process reaped"
    );
}

#[test]
fn provider_does_not_grant_host_imports() {
    let request = ProviderRequest {
        operation: ProviderOperation::Search,
        search: Some(SearchQuery {
            java_release: "1.21.1".into(),
            text: String::new(),
            format: None,
            mod_namespace: None,
            cursor: None,
            limit: 10,
        }),
        key: None,
    };
    let error = plan_provider(
        host(),
        include_bytes!("fixtures/denied-import.wasm"),
        &request,
        &["https://www.mcschematic.top".into()],
        cancel(),
    )
    .unwrap_err();
    assert_eq!(error.code, "PLUGIN_PERMISSION_DENIED");
}

#[test]
#[ignore = "requires built target/community-provider.component.wasm"]
fn independent_provider_uses_opaque_keys_and_its_own_origin() {
    let component = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/community-provider.component.wasm"),
    )
    .unwrap();
    let override_host = std::env::var_os("COBBLE_TEST_PROVIDER_HOST").map(std::path::PathBuf::from);
    let mut session = ProviderSession::open(
        override_host.as_deref().unwrap_or_else(|| host()),
        &component,
        cancel(),
    )
    .unwrap();
    let origins = vec!["https://community.example".into()];
    let mut request = ProviderRequest {
        operation: ProviderOperation::Search,
        search: Some(SearchQuery {
            java_release: "1.21.1".into(),
            text: "hut".into(),
            format: None,
            mod_namespace: None,
            cursor: None,
            limit: 20,
        }),
        key: None,
    };
    for operation in [
        ProviderOperation::Search,
        ProviderOperation::Detail,
        ProviderOperation::Download,
    ] {
        request.operation = operation.clone();
        if operation != ProviderOperation::Search {
            request.search = None;
            request.key = Some(ResourceKey {
                id: "community-hut-a".into(),
                version: 7,
            });
        }
        let plan = session.plan(&request, &origins, cancel()).unwrap();
        assert_eq!(plan.len(), 1);
        let output = session
            .normalize(
                &request,
                &[GetResponse {
                    status: 200,
                    body: b"community-original-fixture".to_vec(),
                }],
                &origins,
                cancel(),
            )
            .unwrap();
        match operation {
            ProviderOperation::Search => assert_eq!(output.items[0].id, "community-hut-a"),
            ProviderOperation::Detail => {
                let detail = output.detail.unwrap();
                assert_eq!(detail.version, 7);
                assert_eq!(detail.license.as_deref(), Some("MIT"));
            }
            ProviderOperation::Download => assert_eq!(
                output.download.unwrap().url,
                "https://community.example/files/hut.schem"
            ),
        }
    }
    request.operation = ProviderOperation::Search;
    request.key = None;
    request.search = Some(SearchQuery {
        java_release: "1.21.1".into(),
        text: "cross-origin".into(),
        format: None,
        mod_namespace: None,
        cursor: None,
        limit: 20,
    });
    assert!(session.plan(&request, &origins, cancel()).is_err());
    session.close();
    assert!(session.has_exited().unwrap());
}

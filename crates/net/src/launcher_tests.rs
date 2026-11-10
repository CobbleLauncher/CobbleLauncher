// SPDX-License-Identifier: MIT
use super::*;
use crate::tests::Server;
fn client(server: &Server) -> Broker {
    let mut broker = Broker::for_launcher(BrokerConfig::default()).unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .add_root_certificate(
            reqwest::Certificate::from_pem(include_bytes!("../tests/fixtures/synthetic-root.pem"))
                .unwrap(),
        )
        .resolve("one.fixture.invalid", server.address)
        .resolve("two.fixture.invalid", server.address)
        .build()
        .unwrap();
    Arc::get_mut(&mut broker.inner).unwrap().launcher_client = Some(client);
    broker
}
#[tokio::test]
async fn launcher_transport_preserves_metadata_and_body_limit() {
    let server = Server::new(|_| {
        vec![(
            Duration::ZERO,
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nETag: \"v1\"\r\n\r\nhello".to_vec(),
        )]
    })
    .await;
    let broker = client(&server);
    assert!(
        !Broker::new(BrokerConfig::default())
            .unwrap()
            .uses_system_network()
    );
    let response = broker
        .get("https://one.fixture.invalid/test", None, 5)
        .await
        .unwrap();
    assert_eq!(response.body, b"hello");
    assert_eq!(response.metadata.etag.as_deref(), Some("\"v1\""));
    assert_eq!(
        broker
            .get("https://one.fixture.invalid/test", None, 4)
            .await
            .err(),
        Some(NetError::BodyLimit)
    );
}
#[tokio::test]
async fn launcher_transport_streams_artifact_and_verifies_size() {
    let server = Server::new(|_| {
        vec![(
            Duration::ZERO,
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello".to_vec(),
        )]
    })
    .await;
    let broker = client(&server);
    let file =
        std::env::temp_dir().join(format!("cobble-native-route-{}.part", std::process::id()));
    let result = broker
        .artifact_transfer(
            crate::ArtifactTransferRequest {
                url: "https://one.fixture.invalid/test".into(),
                credential: None,
                expected_size: 5,
                max_bytes: 5,
                mode: crate::TransferMode::Full,
            },
            std::fs::File::create(&file).unwrap(),
            crate::TransferCancellation::default(),
        )
        .await
        .unwrap();
    drop(result);
    assert_eq!(std::fs::read(&file).unwrap(), b"hello");
    std::fs::remove_file(file).unwrap();
}

#[tokio::test]
async fn launcher_transport_strips_credentials_after_origin_change() {
    let server=Server::new(|request| {
        let body=if request.starts_with("GET /v1/start ") { b"HTTP/1.1 302 Found\r\nLocation: https://two.fixture.invalid/v1/end\r\nContent-Length: 0\r\n\r\n".to_vec() } else { b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec() };
        vec![(Duration::ZERO,body)]
    }).await;
    let broker = client(&server);
    let id = broker
        .register_credentials(
            crate::CredentialScope::new("https://one.fixture.invalid", "/v1").unwrap(),
            crate::ProviderCredentials {
                bearer: Some("fixture-only".into()),
                api_key: None,
                cookie: None,
            },
        )
        .unwrap();
    let response = broker
        .get("https://one.fixture.invalid/v1/start", Some(id), 2)
        .await
        .unwrap();
    assert_eq!(response.body, b"ok");
    let requests = server.captured();
    assert!(requests[0].contains("Bearer fixture-only"));
    assert!(!requests[1].contains("fixture-only"));
}
#[tokio::test]
async fn launcher_transport_revalidates_without_reading_a_body() {
    let server = Server::new(|_| {
        vec![(
            Duration::ZERO,
            b"HTTP/1.1 304 Not Modified\r\nETag: \"cached\"\r\n\r\n".to_vec(),
        )]
    })
    .await;
    let broker = client(&server);
    let response = broker
        .get_revalidated(
            "https://one.fixture.invalid/metadata",
            5,
            Revalidation {
                etag: Some("\"cached\"".into()),
                last_modified: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(response.metadata.status, 304);
    assert!(response.body.is_empty());
    assert!(server.captured()[0].contains("cached"));
}

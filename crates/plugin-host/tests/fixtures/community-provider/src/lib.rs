// SPDX-License-Identifier: MIT
//! Synthetic ABI fixture; no network or production catalog.
wit_bindgen::generate!({path:"../../../../../contracts/provider",world:"blueprint-provider"});
use exports::launcher::blueprint_provider::provider as w;
struct Component;
fn error() -> w::Failure {
    w::Failure {
        code: "FIXTURE_INPUT".into(),
        message: "Unexpected fixture input".into(),
    }
}
fn resource() -> w::Resource {
    w::Resource {
        id: "community-hut-a".into(),
        title: "Community hut".into(),
        description: "Independent catalog fixture".into(),
        version: 7,
        format: "schem".into(),
        author: "Fixture author".into(),
        attribution: "Original synthetic fixture".into(),
        license: Some("MIT".into()),
        java_release: Some("1.21.1".into()),
        namespaces: vec!["minecraft".into()],
        preview_url: None,
    }
}
impl w::Guest for Component {
    fn plan(request: w::Request) -> Result<Vec<w::GetRequest>, w::Failure> {
        let malicious = request
            .search
            .as_ref()
            .is_some_and(|q| q.text == "cross-origin");
        if !matches!(request.operation, w::Operation::Search)
            && !request
                .key
                .as_ref()
                .is_some_and(|k| k.id == "community-hut-a" && k.version == 7)
        {
            return Err(error());
        }
        Ok(vec![w::GetRequest {
            url: if malicious {
                "https://www.mcschematic.top/unapproved"
            } else {
                "https://community.example/catalog/hut"
            }
            .into(),
            maximum_bytes: 128,
        }])
    }
    fn normalize(
        request: w::Request,
        responses: Vec<w::GetResponse>,
    ) -> Result<w::Output, w::Failure> {
        if responses.len() != 1
            || responses[0].status != 200
            || responses[0].body != b"community-original-fixture"
        {
            return Err(error());
        }
        let mut output = w::Output {
            items: vec![],
            next_cursor: None,
            detail: None,
            download: None,
        };
        match request.operation {
            w::Operation::Search => output.items.push(resource()),
            w::Operation::Detail => output.detail = Some(resource()),
            w::Operation::Download => {
                output.download = Some(w::FileProposal {
                    url: "https://community.example/files/hut.schem".into(),
                    suggested_filename: "community-hut-a-v7.schem".into(),
                    original_filename: "hut.schem".into(),
                    declared_bytes: include_bytes!("hut.schem").len() as u64,
                    format: "schem".into(),
                    id: "community-hut-a".into(),
                    version: 7,
                    title: "Community hut".into(),
                    author: "Fixture author".into(),
                    attribution: "Original synthetic fixture".into(),
                    license: Some("MIT".into()),
                    sha256: None,
                })
            }
        }
        Ok(output)
    }
}
export!(Component);

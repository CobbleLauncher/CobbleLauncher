// SPDX-License-Identifier: MIT
use super::*;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn decode(value: &str) -> Result<Vec<u8>, String> {
    if value.len() > 4096 || !value.len().is_multiple_of(2) {
        return Err("MESH_ENVIRONMENT_TRANSFER_INVALID".into());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|value| u8::from_str_radix(value, 16).ok())
                .ok_or_else(|| "MESH_ENVIRONMENT_TRANSFER_INVALID".into())
        })
        .collect()
}
pub(super) fn upload(
    client: &client::Client,
    group_id: u64,
    bytes: &[u8],
) -> Result<String, String> {
    let result=client.request(json!({"op":"environment_upload_begin","byte_count":bytes.len(),"sha256":cobble_plugin_host::digest(bytes)}))?;
    let id = result["transfer_id"]
        .as_str()
        .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?;
    let result = (|| {
        for (offset, chunk) in bytes.chunks(2048).enumerate() {
            client.request(json!({"op":"environment_upload_chunk","transfer_id":id,"offset":offset*2048,"hex":hex(chunk)}))?;
        }
        let result = client.request(
            json!({"op":"group_environment_publish","group_id":group_id,"transfer_id":id}),
        )?;
        result["invitation"]
            .as_str()
            .filter(|value| value.len() <= 6000)
            .map(str::to_owned)
            .ok_or_else(|| "MESH_ENVIRONMENT_TRANSFER_INVALID".into())
    })();
    let _ = client.request(json!({"op":"environment_release","transfer_id":id}));
    result
}
pub(super) fn receive(invitation: String) -> Result<(document::Document, String, String), String> {
    let client = {
        let mut broker = broker()
            .lock()
            .map_err(|_| "MESH_ENVIRONMENT_UNAVAILABLE")?;
        enabled::require(&mut broker)?;
        let client = broker.shared_client()?;
        let directory = broker
            .temporary_identity
            .as_ref()
            .map(|root| root.path().join("identity"))
            .map(Ok)
            .unwrap_or_else(app::private_directory)?;
        client.request(
            json!({"op":"app_initialize","directory":directory,"display_name":"Player"}),
        )?;
        client
    };
    let value=client.request(json!({"op":"environment_fetch","invitation":invitation,"route_policy":overview::saved_relay().route_policy}))?;
    let id = value["transfer_id"]
        .as_str()
        .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?;
    let result = (|| {
        let size = value["reference"]["byte_count"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .filter(|size| *size > 0 && *size <= 1024 * 1024)
            .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?;
        let digest = value["reference"]["sha256"]
            .as_str()
            .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?;
        let host = value["host_peer_id"]
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= 128)
            .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?;
        let mut bytes = Vec::with_capacity(size);
        while bytes.len() < size {
            let chunk = client.request(
                json!({"op":"environment_read","transfer_id":id,"offset":bytes.len(),"limit":2048}),
            )?;
            let chunk = decode(
                chunk["hex"]
                    .as_str()
                    .ok_or("MESH_ENVIRONMENT_TRANSFER_INVALID")?,
            )?;
            if chunk.is_empty() || bytes.len() + chunk.len() > size {
                return Err("MESH_ENVIRONMENT_TRANSFER_INVALID".into());
            }
            bytes.extend(chunk);
        }
        if cobble_plugin_host::digest(&bytes) != digest {
            return Err("MESH_ENVIRONMENT_TRANSFER_INVALID".into());
        }
        let document: document::Document =
            serde_json::from_slice(&bytes).map_err(|_| "MESH_ENVIRONMENT_DOCUMENT_INVALID")?;
        document::validate(&document)?;
        Ok((document, host.to_owned(), digest.to_owned()))
    })();
    let _ = client.request(json!({"op":"environment_release","transfer_id":id}));
    result
}

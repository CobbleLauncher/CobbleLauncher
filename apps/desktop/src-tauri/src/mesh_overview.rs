// SPDX-License-Identifier: MIT
use super::*;
#[derive(Clone, Serialize)]
pub struct RelaySelection {
    pub address: Option<String>,
    pub source: Option<&'static str>,
    pub state: &'static str,
}
fn relay_path() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Configuration folder unavailable")?
        .join("CobbleLauncher/mesh-relay.json"))
}
pub(super) fn saved_relay() -> RelaySelection {
    let address = relay_path().ok().and_then(|path| read_saved_relay(&path));
    RelaySelection {
        state: if address.is_some() {
            "configured"
        } else {
            "unconfigured"
        },
        source: address.as_ref().map(|_| "last_successful"),
        address,
    }
}
fn read_saved_relay(path: &std::path::Path) -> Option<String> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return None;
        }
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(4097)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4096 {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    if value.get("schema_version")?.as_u64() != Some(1) {
        return None;
    }
    value
        .get("relay")?
        .as_str()
        .filter(|s| valid_relay(s))
        .map(str::to_owned)
}

pub(super) fn valid_relay(value: &str) -> bool {
    value.len() <= 1024
        && (value.starts_with("/ip4/") || value.starts_with("/dns4/"))
        && !value.chars().any(char::is_control)
}
pub(super) fn save_relay(address: &str) -> Result<(), String> {
    if !valid_relay(address) {
        return Err("Invalid relay selection".into());
    }
    save_relay_at(&relay_path()?, address)
}
fn save_relay_at(path: &std::path::Path, address: &str) -> Result<(), String> {
    if !valid_relay(address) {
        return Err("Invalid relay selection".into());
    }
    use std::io::Write;
    let parent = path.parent().ok_or("Configuration folder unavailable")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let metadata = std::fs::symlink_metadata(parent).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Configuration folder must be a real directory".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Configuration folder must be a real directory".into());
        }
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .write_all(
            &serde_json::to_vec(&json!({"schema_version":1,"relay":address}))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
#[derive(Serialize)]
pub struct HostOptions {
    pub game_id: String,
    pub lan: super::lan::Lan,
    pub relay: RelaySelection,
}
#[derive(Serialize)]
pub struct Capabilities {
    pub relay: bool,
    pub direct: bool,
    pub hole_punch: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedNode {
    pub peer_id: String,
    pub endpoint: String,
    pub relay: bool,
    pub discovery: bool,
    pub expires_at: u64,
    pub evidence: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfiguredService {
    pub id: String,
    pub kind: String,
    pub endpoint: String,
    pub state: String,
    pub discovery: String,
    #[serde(default)]
    pub game_relay: bool,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Discovery {
    pub state: String,
    pub source: String,
    pub total: u32,
    pub nodes: Vec<PublishedNode>,
    #[serde(default)]
    pub configured_services: Vec<ConfiguredService>,
}
#[derive(Serialize)]
pub struct Overview {
    pub available: bool,
    pub reason: Option<String>,
    pub can_host: bool,
    pub component_state: &'static str,
    pub mode: &'static str,
    pub discovery_state: String,
    pub discovery_source: Option<String>,
    pub discovery_total: u32,
    pub nodes: Vec<PublishedNode>,
    pub configured_services: Vec<ConfiguredService>,
    pub neighbors: Vec<serde_json::Value>,
    pub capabilities: Capabilities,
    pub relay: RelaySelection,
    pub session: Option<MeshSession>,
}
#[tauri::command]
pub async fn mesh_host_options(app: AppHandle, game_id: String) -> Result<HostOptions, String> {
    let game = running_game(&app, &game_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(HostOptions {
            game_id,
            lan: super::lan::discover(&game),
            relay: saved_relay(),
        })
    })
    .await
    .map_err(|_| "LAN lookup failed")?
}
fn view(broker: &mut Broker) -> Result<Overview, String> {
    let online = enabled::is_enabled(broker);
    let available = online && helper_path().is_ok_and(|path| client::helper_file(&path));
    let session = broker.status()?.session;
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock unavailable")?
        .as_secs();
    let nodes = broker
        .discovery
        .nodes
        .iter()
        .filter(|node| node.expires_at > at)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Overview {
        available,
        reason: if !online {
            Some("MESH_OFFLINE".into())
        } else if !available {
            Some("Mesh companion is not included in this installation".into())
        } else if broker.discovery.state == "unavailable" {
            Some("Discovery service is unavailable".into())
        } else {
            None
        },
        can_host: available && cfg!(windows),
        component_state: if !available {
            "missing"
        } else if broker.client.is_some()
            || broker.discovery_client.is_some()
            || broker.social_client.is_some()
        {
            "running"
        } else {
            "idle"
        },
        mode: "explicit_relay",
        discovery_state: if broker.discovery.state.is_empty() {
            "inactive".into()
        } else {
            broker.discovery.state.clone()
        },
        discovery_source: (!broker.discovery.source.is_empty())
            .then(|| broker.discovery.source.clone()),
        discovery_total: if nodes.len() == broker.discovery.nodes.len() {
            broker.discovery.total
        } else {
            nodes.len() as u32
        },
        nodes,
        configured_services: broker.discovery.configured_services.clone(),
        neighbors: Vec::new(),
        capabilities: Capabilities {
            relay: available,
            direct: false,
            hole_punch: false,
        },
        relay: saved_relay(),
        session,
    })
}
#[tauri::command]
pub async fn mesh_overview() -> Result<Overview, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut state = broker().lock().map_err(|_| "Mesh status unavailable")?;
        view(&mut state)
    })
    .await
    .map_err(|_| "Mesh status lookup failed")?
}
fn valid_discovery(discovery: &Discovery) -> bool {
    matches!(discovery.state.as_str(), "published" | "unavailable")
        && discovery.source == "https://cobble-mesh-v0.yuzhes.com/v1/bootstrap"
        && discovery.nodes.len() <= 8
        && discovery.total <= 160
        && discovery.nodes.iter().all(|node| {
            node.peer_id.len() <= 128
                && node.endpoint.len() <= 512
                && valid_relay(&node.endpoint)
                && node.evidence == "signed_advertisement"
        })
        && discovery.configured_services.len() <= 8
        && discovery.configured_services.iter().all(|service| {
            !service.id.is_empty()
                && service.id.len() <= 64
                && service
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                && service.endpoint.len() <= 1024
                && matches!(
                    service.state.as_str(),
                    "configured" | "authenticated" | "unavailable"
                )
                && matches!(
                    service.discovery.as_str(),
                    "available" | "unavailable" | "not_checked"
                )
                && match service.kind.as_str() {
                    "bootstrap" => service.endpoint == discovery.source && !service.game_relay,
                    "relay" => {
                        valid_relay(&service.endpoint)
                            && (!service.game_relay || service.state == "authenticated")
                    }
                    _ => false,
                }
        })
}
#[tauri::command]
pub async fn mesh_discover() -> Result<Overview, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut broker = broker().lock().map_err(|_| "Mesh status unavailable")?;
        enabled::require(&mut broker)?;
        let attempt = (|| -> Result<Discovery, String> {
            if broker.discovery_client.is_none() {
                broker.discovery_client = Some(client::Client::spawn(&helper_path()?)?);
            }
            let value = broker
                .discovery_client
                .as_mut()
                .ok_or("Discovery helper unavailable")?
                .request(json!({"op":"discover"}))?;
            let discovery: Discovery =
                serde_json::from_value(value).map_err(|_| "Invalid discovery response")?;
            if !valid_discovery(&discovery) {
                return Err("Invalid discovery response".into());
            }
            Ok(discovery)
        })();
        broker.discovery_checked_at = Some(Instant::now());
        match attempt {
            Ok(discovery) => broker.discovery = discovery,
            Err(_) => {
                broker.discovery_client.take();
                broker.discovery = Discovery {
                    state: "unavailable".into(),
                    ..Default::default()
                };
            }
        }
        view(&mut broker)
    })
    .await
    .map_err(|_| "Discovery lookup failed")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_relay_is_bounded_versioned_and_never_replaces_with_invalid_input() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("relay.json");
        let relay = "/ip4/8.8.8.8/tcp/4001/p2p/fixture";
        assert!(read_saved_relay(&path).is_none());
        save_relay_at(&path, relay).unwrap();
        assert_eq!(read_saved_relay(&path).as_deref(), Some(relay));
        let bytes = std::fs::read(&path).unwrap();
        assert!(save_relay_at(&path, "http://invalid").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"broken JSON").unwrap();
        assert!(read_saved_relay(&path).is_none());
        std::fs::write(&path, vec![b'x'; 4097]).unwrap();
        assert!(read_saved_relay(&path).is_none());
    }
    #[test]
    fn idle_overview_does_not_invent_neighbors_or_direct_game_capability() {
        let overview = view(&mut Broker::default()).unwrap();
        assert!(overview.nodes.is_empty());
        assert!(overview.neighbors.is_empty());
        assert_eq!(overview.discovery_state, "inactive");
        assert!(!overview.capabilities.direct);
        assert!(!overview.capabilities.hole_punch);
    }
    #[test]
    fn configured_seed_is_not_a_signed_node_or_relay_capability() {
        let discovery:Discovery=serde_json::from_value(json!({"state":"published","source":"https://cobble-mesh-v0.yuzhes.com/v1/bootstrap","total":0,"nodes":[],"configured_services":[{"id":"official-relay","kind":"relay","endpoint":"/dns4/seed.example/tcp/443/p2p/fixture","state":"authenticated","discovery":"unavailable","game_relay":false}]})).unwrap();
        assert!(valid_discovery(&discovery));
        let mut broker = Broker::default();
        broker.discovery = discovery;
        let overview = view(&mut broker).unwrap();
        assert!(overview.nodes.is_empty());
        assert_eq!(overview.discovery_total, 0);
        assert_eq!(overview.configured_services.len(), 1);
        assert!(!overview.configured_services[0].game_relay);
    }
    #[test]
    fn unprobed_seed_cannot_claim_game_relay_and_bootstrap_cannot_be_relay() {
        let mut discovery:Discovery=serde_json::from_value(json!({"state":"published","source":"https://cobble-mesh-v0.yuzhes.com/v1/bootstrap","total":0,"nodes":[],"configured_services":[{"id":"official-relay","kind":"relay","endpoint":"/dns4/seed.example/tcp/443/p2p/fixture","state":"configured","discovery":"not_checked","game_relay":true}]})).unwrap();
        assert!(!valid_discovery(&discovery));
        discovery.configured_services[0].state = "authenticated".into();
        assert!(valid_discovery(&discovery));
        discovery.configured_services[0].kind = "bootstrap".into();
        discovery.configured_services[0].endpoint = discovery.source.clone();
        assert!(!valid_discovery(&discovery));
    }
}

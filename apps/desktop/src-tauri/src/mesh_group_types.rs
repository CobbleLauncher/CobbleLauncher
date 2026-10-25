// SPDX-License-Identifier: MIT
use super::*;
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub peer_id: String,
    pub state: String,
    pub is_host: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WireGroup {
    pub group_id: u64,
    pub role: String,
    pub state: String,
    pub host_peer_id: String,
    pub capacity: u32,
    pub members: Vec<Member>,
    pub membership_complete: bool,
    pub invitation_expires_at: Option<u64>,
    pub admission_open: bool,
    pub expires_at: u64,
    pub route: String,
    #[serde(default)]
    pub bridge_address: Option<String>,
}
impl WireGroup {
    pub fn active(&self) -> bool {
        matches!(self.state.as_str(), "hosting" | "bridge_ready")
    }
    pub fn validate(&self) -> Result<(), String> {
        let peer = |value: &str| {
            !value.is_empty()
                && value.len() <= 128
                && value.bytes().all(|b| b.is_ascii_alphanumeric())
        };
        let mut ids = std::collections::HashSet::new();
        if self.group_id == 0
            || self.group_id > 9_007_199_254_740_991
            || self.capacity != 3
            || !peer(&self.host_peer_id)
            || !matches!(self.role.as_str(), "host" | "guest")
            || !matches!(
                self.state.as_str(),
                "hosting" | "bridge_ready" | "ended" | "failed"
            )
            || !matches!(
                self.route.as_str(),
                "unknown" | "direct" | "relay" | "explicit_relay"
            )
            || self.members.len() > self.capacity as usize
            || self.members.iter().any(|member| {
                !peer(&member.peer_id)
                    || member.state != "connected"
                    || member.is_host != (member.peer_id == self.host_peer_id)
                    || !ids.insert(&member.peer_id)
            })
            || (self.active() && !self.members.iter().any(|member| member.is_host))
            || (self.role == "host" && !self.membership_complete)
            || (self.role == "guest" && self.membership_complete)
            || (self.state == "hosting" && self.role != "host")
            || (self.state == "bridge_ready" && self.role != "guest")
            || self.expires_at == 0
            || self.expires_at > 9_007_199_254_740_991
        {
            return Err("MESH_GROUP_INVALID_RESPONSE".into());
        }
        if let Some(address) = &self.bridge_address {
            validate_bridge(address)?;
        }
        if self.state == "bridge_ready" && self.bridge_address.is_none() {
            return Err("MESH_GROUP_INVALID_RESPONSE".into());
        }
        Ok(())
    }
}
pub(super) fn validate_bridge(value: &str) -> Result<(), String> {
    let address: std::net::SocketAddr = value.parse().map_err(|_| "MESH_GROUP_INVALID_RESPONSE")?;
    if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) || address.port() < 1024
    {
        return Err("MESH_GROUP_INVALID_RESPONSE".into());
    }
    Ok(())
}
#[derive(Clone, Serialize)]
pub struct GroupSnapshot {
    pub group_id: u64,
    pub game_id: String,
    pub game_name: String,
    pub role: String,
    pub state: String,
    pub host_peer_id: String,
    pub capacity: u32,
    pub members: Vec<Member>,
    pub membership_complete: bool,
    pub invitation_expires_at: Option<u64>,
    pub admission_open: bool,
    pub expires_at: u64,
    pub route: String,
    pub bridge_address: Option<String>,
}
impl GroupSnapshot {
    pub(super) fn new(id: u64, game: &sessions::Session, name: &str, wire: &WireGroup) -> Self {
        Self {
            group_id: id,
            game_id: game.game_id.clone(),
            game_name: name.into(),
            role: wire.role.clone(),
            state: wire.state.clone(),
            host_peer_id: wire.host_peer_id.clone(),
            capacity: wire.capacity,
            members: wire.members.clone(),
            membership_complete: wire.membership_complete,
            invitation_expires_at: wire.invitation_expires_at,
            admission_open: wire.admission_open,
            expires_at: wire.expires_at,
            route: wire.route.clone(),
            bridge_address: wire.bridge_address.clone(),
        }
    }
}
#[derive(Clone, Serialize)]
pub struct View {
    pub generation: u64,
    pub group: Option<GroupSnapshot>,
}
#[derive(Serialize)]
pub struct InvitationResult {
    #[serde(flatten)]
    pub view: View,
    pub invitation: String,
}
#[derive(Serialize)]
pub struct JoinResult {
    #[serde(flatten)]
    pub view: View,
    pub bridge_address: String,
}
pub(super) fn decode(value: &Value) -> Result<WireGroup, String> {
    let wire: WireGroup = serde_json::from_value(
        value
            .get("group")
            .cloned()
            .ok_or("MESH_GROUP_INVALID_RESPONSE")?,
    )
    .map_err(|_| "MESH_GROUP_INVALID_RESPONSE")?;
    wire.validate()?;
    Ok(wire)
}
pub(super) fn invitation(value: &Value) -> Result<String, String> {
    value
        .get("invitation")
        .and_then(Value::as_str)
        .filter(|text| {
            !text.is_empty() && text.len() <= 6000 && !text.chars().any(char::is_control)
        })
        .map(str::to_owned)
        .ok_or_else(|| "MESH_GROUP_INVALID_RESPONSE".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn group() -> WireGroup {
        serde_json::from_value(json!({"group_id":1,"role":"host","state":"hosting","host_peer_id":"12D3Host","capacity":3,"members":[{"peer_id":"12D3Host","state":"connected","is_host":true}],"membership_complete":true,"invitation_expires_at":20,"admission_open":true,"expires_at":30,"route":"explicit_relay"})).unwrap()
    }
    #[test]
    fn observed_membership_is_bounded_unique_and_never_guessed_for_guest() {
        let mut wire = group();
        assert!(wire.validate().is_ok());
        wire.members.push(wire.members[0].clone());
        assert!(wire.validate().is_err());
        let mut wire = group();
        wire.role = "guest".into();
        wire.state = "bridge_ready".into();
        wire.bridge_address = Some("127.0.0.1:25566".into());
        assert!(wire.validate().is_err());
        wire.membership_complete = false;
        assert!(wire.validate().is_ok());
        wire.bridge_address = Some("0.0.0.0:25566".into());
        assert!(wire.validate().is_err());
    }
}

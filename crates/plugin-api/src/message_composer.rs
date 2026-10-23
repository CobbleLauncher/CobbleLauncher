// SPDX-License-Identifier: MIT
//! Public, user-selected snapshots for pure message draft generation, never Send.
use crate::{PluginError, PluginManifest};
use serde::{Deserialize, Serialize};
pub const MESSAGE_COMPOSER_API_VERSION: &str = "0.1.0";
pub const MAX_MESSAGE_BYTES: usize = 1024;
pub const MAX_NOTE_BYTES: usize = 512;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposerPermission {
    Draft,
    PublicIdentity,
    SelectedPeer,
    SelectedSession,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicIdentity {
    pub id: String,
    pub display_name: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerSource {
    Contact,
    Discovery,
    Session,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedPeer {
    pub identity: PublicIdentity,
    pub source: PeerSource,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedSession {
    pub game_name: String,
    pub game_version: String,
    pub loader: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageComposerContext {
    pub locale: String,
    pub public_self: Option<PublicIdentity>,
    pub selected_peer: Option<SelectedPeer>,
    pub selected_session: Option<SelectedSession>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageComposerRequest {
    pub context: MessageComposerContext,
    pub note: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageDraft {
    pub text: String,
}
fn plain(value: &str, max: usize, multiline: bool) -> bool {
    value.len() <= max
        && !value
            .chars()
            .any(|c| c.is_control() && !(multiline && c == '\n'))
}
fn invalid() -> PluginError {
    PluginError::new(
        "PLUGIN_INVALID_MESSAGE",
        "Invalid or oversized message draft context",
    )
}
impl PublicIdentity {
    fn validate(&self) -> Result<(), PluginError> {
        if self.id.is_empty()
            || !plain(&self.id, 256, false)
            || self.id.chars().any(char::is_whitespace)
            || self
                .display_name
                .as_ref()
                .is_some_and(|name| name.is_empty() || !plain(name, 128, false))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
impl MessageComposerRequest {
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginError> {
        manifest.validate()?;
        if manifest.api_version != MESSAGE_COMPOSER_API_VERSION
            || manifest.capabilities != ["message-composer"]
        {
            return Err(PluginError::new(
                "PLUGIN_UNSUPPORTED_API",
                "Expected message-composer interface",
            ));
        }
        for (present, permission) in [
            (true, ComposerPermission::Draft),
            (
                self.context.public_self.is_some(),
                ComposerPermission::PublicIdentity,
            ),
            (
                self.context.selected_peer.is_some(),
                ComposerPermission::SelectedPeer,
            ),
            (
                self.context.selected_session.is_some(),
                ComposerPermission::SelectedSession,
            ),
        ] {
            if present && !manifest.composer_permissions.contains(&permission) {
                return Err(PluginError::new(
                    "PLUGIN_PERMISSION_DENIED",
                    "Composer did not declare this public context scope",
                ));
            }
        }
        if !matches!(self.context.locale.as_str(), "en" | "zh-CN")
            || !plain(&self.note, MAX_NOTE_BYTES, true)
        {
            return Err(invalid());
        }
        if let Some(identity) = &self.context.public_self {
            identity.validate()?;
        }
        if let Some(peer) = &self.context.selected_peer {
            peer.identity.validate()?;
        }
        if let Some(session) = &self.context.selected_session
            && (session.game_name.is_empty()
                || !plain(&session.game_name, 256, false)
                || session.game_version.is_empty()
                || !plain(&session.game_version, 64, false)
                || session
                    .loader
                    .as_ref()
                    .is_some_and(|loader| loader.is_empty() || !plain(loader, 64, false)))
        {
            return Err(invalid());
        }
        if serde_json::to_vec(self).map_err(|_| invalid())?.len() > 8192 {
            return Err(invalid());
        }
        Ok(())
    }
}
impl MessageDraft {
    pub fn validate(&self) -> Result<(), PluginError> {
        if self.text.trim().is_empty() || !plain(&self.text, MAX_MESSAGE_BYTES, true) {
            return Err(invalid());
        }
        Ok(())
    }
}
impl PluginManifest {
    pub(crate) fn validate_composer_permissions(&self) -> Result<(), PluginError> {
        let unique: std::collections::BTreeSet<_> = self.composer_permissions.iter().collect();
        let composer = self.api_version == MESSAGE_COMPOSER_API_VERSION
            && self.capabilities == ["message-composer"];
        if unique.len() != self.composer_permissions.len()
            || self.composer_permissions.len() > 4
            || (!composer && !self.composer_permissions.is_empty())
            || (composer
                && !self
                    .composer_permissions
                    .contains(&ComposerPermission::Draft))
        {
            return Err(PluginError::new(
                "PLUGIN_INVALID_PACKAGE",
                "Invalid message composer permission declaration",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> PluginManifest {
        serde_json::from_value(serde_json::json!({"id":"composer-fixture","name":"Composer","version":"1.0.0","publisher":"Original fixture","api_version":"0.1.0","capabilities":["message-composer"],"file_extensions":[],"composer_permissions":["draft","selected_session"],"component_sha256":"a".repeat(64)})).unwrap()
    }
    fn request() -> MessageComposerRequest {
        MessageComposerRequest {
            context: MessageComposerContext {
                locale: "en".into(),
                public_self: None,
                selected_peer: None,
                selected_session: Some(SelectedSession {
                    game_name: "Example game".into(),
                    game_version: "1.21.1".into(),
                    loader: Some("fabric".into()),
                }),
            },
            note: "Meet at the village.\nBring blocks.".into(),
        }
    }
    #[test]
    fn context_scopes_are_explicit_and_never_implied_by_an_interface() {
        let mut manifest = manifest();
        let mut request = request();
        request.validate(&manifest).unwrap();
        request.context.public_self = Some(PublicIdentity {
            id: "public-peer-id".into(),
            display_name: Some("Alex".into()),
        });
        assert_eq!(
            request.validate(&manifest).unwrap_err().code,
            "PLUGIN_PERMISSION_DENIED"
        );
        manifest
            .composer_permissions
            .push(ComposerPermission::PublicIdentity);
        request.validate(&manifest).unwrap();
        request.context.selected_peer = Some(SelectedPeer {
            identity: PublicIdentity {
                id: "selected-peer".into(),
                display_name: None,
            },
            source: PeerSource::Contact,
        });
        assert_eq!(
            request.validate(&manifest).unwrap_err().code,
            "PLUGIN_PERMISSION_DENIED"
        );
        manifest
            .composer_permissions
            .push(ComposerPermission::SelectedPeer);
        request.validate(&manifest).unwrap();
        manifest
            .composer_permissions
            .push(ComposerPermission::Draft);
        assert!(manifest.validate().is_err());
    }
    #[test]
    fn drafts_notes_and_metadata_are_bounded_plain_text_without_targets() {
        let manifest = manifest();
        let mut request = request();
        request.note = "x".repeat(513);
        assert!(request.validate(&manifest).is_err());
        for text in ["".into(), "x".repeat(1025), "bad\0text".into()] {
            assert!(MessageDraft { text }.validate().is_err());
        }
        MessageDraft {
            text: "A real draft\nNo automatic send".into(),
        }
        .validate()
        .unwrap();
        assert!(
            serde_json::from_value::<MessageDraft>(
                serde_json::json!({"text":"hello","recipient":"other-peer"})
            )
            .is_err()
        );
        let mut other = manifest.clone();
        other.capabilities = vec!["report".into()];
        other.file_extensions = vec!["txt".into()];
        assert!(other.validate().is_err());
    }
}

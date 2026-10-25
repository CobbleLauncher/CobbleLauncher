// SPDX-License-Identifier: MIT
//! Bounded application event names and payloads. Namespaces are local consent
//! scopes; they do not authenticate a remote plugin or its publisher.
use crate::PluginError;
use serde_json::Value;
use std::io::{self, Write};

pub const MAX_MESH_NAMESPACES: usize = 8;
pub const MAX_MESH_NAMESPACE_BYTES: usize = 96;
pub const MAX_MESH_EVENT_BYTES: usize = 64;
pub const MAX_MESH_PAYLOAD_BYTES: usize = 1024;
pub const MAX_MESH_PAYLOAD_DEPTH: usize = 8;
pub const RESERVED_MESH_EVENTS: &[&str] = &[
    "connect",
    "connect_error",
    "disconnect",
    "disconnecting",
    "error",
    "reconnect",
    "reconnect_attempt",
    "reconnect_error",
    "reconnect_failed",
    "newlistener",
    "removelistener",
    "companion_exited",
    "network_state_changed",
    "group_state_changed",
    "app_events_changed",
];

pub fn valid_mesh_namespace(value: &str) -> bool {
    value.len() <= MAX_MESH_NAMESPACE_BYTES
        && !value.starts_with("cobble.")
        && value.contains('.')
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 63
                && segment.as_bytes()[0].is_ascii_lowercase()
                && segment
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

pub fn valid_mesh_event(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MESH_EVENT_BYTES
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'.' | b':' | b'-')
        })
        && !RESERVED_MESH_EVENTS.contains(&value)
}

/// The complete encoded JSON value (including quotes and escapes) is at most
/// 1024 UTF-8 bytes. A leaf has depth zero; each array/object adds one level.
pub fn validate_mesh_payload(value: &Value) -> Result<(), PluginError> {
    fn within_depth(value: &Value, depth: usize) -> bool {
        match value {
            Value::Array(values) => {
                depth < MAX_MESH_PAYLOAD_DEPTH
                    && values.iter().all(|value| within_depth(value, depth + 1))
            }
            Value::Object(values) => {
                depth < MAX_MESH_PAYLOAD_DEPTH
                    && values.values().all(|value| within_depth(value, depth + 1))
            }
            _ => true,
        }
    }
    if !within_depth(value, 0) {
        return Err(PluginError::new(
            "PLUGIN_MESH_PAYLOAD_INVALID",
            "Mesh event JSON nesting exceeds eight containers",
        ));
    }
    struct Budget {
        bytes: usize,
    }
    impl Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_MESH_PAYLOAD_BYTES - self.bytes {
                return Err(io::Error::other("mesh payload byte limit"));
            }
            self.bytes += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget { bytes: 0 }, value).map_err(|_| {
        PluginError::new(
            "PLUGIN_MESH_PAYLOAD_TOO_LARGE",
            "Mesh event JSON exceeds 1024 UTF-8 bytes",
        )
    })
}

#[cfg(test)]
#[path = "mesh_events/tests.rs"]
mod tests;

// SPDX-License-Identifier: MIT
//! Owned companion process and bounded, private stdio frames. Never log protocol data.
use process_wrap::std::{ChildWrapper, CommandWrap};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

// Private host-companion frames include bounded file inventories and encoded chunks.
// Peer application events retain their separate 1 KiB payload limit.
pub(super) const MAX_FRAME: usize = 4 * 1024 * 1024;
#[derive(Clone)]
pub(super) struct Client(Arc<Inner>);
type Pending = Arc<Mutex<std::collections::HashMap<u64, mpsc::Sender<Result<Value, ()>>>>>;
type Handler = Arc<dyn Fn(Value) + Send + Sync>;
#[derive(Default)]
struct EventHandlers {
    network: Option<Handler>,
    group: Option<Handler>,
}
type Events = Arc<Mutex<EventHandlers>>;
struct Inner {
    child: Mutex<Box<dyn ChildWrapper>>,
    input: Option<mpsc::SyncSender<(Vec<u8>, mpsc::Sender<bool>)>>,
    pending: Pending,
    events: Events,
    workers: Vec<thread::JoinHandle<()>>,
    next_id: AtomicU64,
}

fn read_frame(reader: &mut impl BufRead) -> Result<Value, ()> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_FRAME as u64 + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() > MAX_FRAME || bytes.last() != Some(&b'\n') {
        return Err(());
    }
    serde_json::from_slice(&bytes).map_err(|_| ())
}

pub(super) fn helper_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

impl Client {
    pub(super) fn spawn(path: &Path) -> Result<Self, String> {
        if !helper_file(path) {
            return Err("当前安装未包含联机组件".into());
        }
        let mut command = Command::new(path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut command = CommandWrap::from(command);
        #[cfg(windows)]
        {
            use process_wrap::std::{CreationFlags, JobObject};
            let mut flags = CreationFlags(Default::default());
            flags.0.0 = 0x0800_0000;
            command.wrap(flags).wrap(JobObject);
        }
        let mut child = command.spawn().map_err(|_| "无法启动联机组件")?;
        let mut input = child.stdin().take().ok_or("联机组件输入不可用")?;
        let output = child.stdout().take().ok_or("联机组件输出不可用")?;
        let (write_tx, write_rx) = mpsc::sync_channel::<(Vec<u8>, mpsc::Sender<bool>)>(64);
        let writer = thread::spawn(move || {
            for (bytes, receipt) in write_rx {
                let ok = input.write_all(&bytes).and_then(|()| input.flush()).is_ok();
                let _ = receipt.send(ok);
                if !ok {
                    break;
                }
            }
        });
        let pending: Pending = Default::default();
        let events: Events = Default::default();
        let reader_pending = pending.clone();
        let reader_events = events.clone();
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let frame = read_frame(&mut output);
                match frame {
                    Ok(frame) => dispatch(frame, &reader_pending, &reader_events),
                    Err(()) => {
                        if let Ok(mut pending) = reader_pending.lock() {
                            for (_, sender) in pending.drain() {
                                let _ = sender.send(Err(()));
                            }
                        }
                        let handlers = reader_events
                            .lock()
                            .map(|events| [events.network.clone(), events.group.clone()])
                            .unwrap_or_default();
                        for handler in handlers.into_iter().flatten() {
                            handler(json!({"event":"companion_exited"}));
                        }
                        break;
                    }
                }
            }
        });
        Ok(Self(Arc::new(Inner {
            child: Mutex::new(child),
            input: Some(write_tx),
            pending,
            events,
            workers: vec![writer, reader],
            next_id: AtomicU64::new(1),
        })))
    }

    pub(super) fn request(&self, command: Value) -> Result<Value, String> {
        let seconds = if command.get("op").and_then(Value::as_str) == Some("environment_fetch") {
            45
        } else {
            20
        };
        self.request_timeout(
            command,
            Duration::from_secs(5),
            Duration::from_secs(seconds),
        )
    }
    pub(super) fn on_event(&self, handler: Arc<dyn Fn(Value) + Send + Sync>) {
        if let Ok(mut events) = self.0.events.lock() {
            events.network = Some(handler);
        }
    }
    pub(super) fn on_group_event(&self, handler: Handler) {
        if let Ok(mut events) = self.0.events.lock() {
            events.group = Some(handler);
        }
    }
    pub(super) fn alive(&self) -> bool {
        self.0
            .child
            .lock()
            .is_ok_and(|mut child| child.try_wait().is_ok_and(|status| status.is_none()))
    }
    pub(super) fn shutdown(&self) {
        let _ = self.request_timeout(
            json!({"op":"shutdown"}),
            Duration::from_secs(1),
            Duration::from_secs(3),
        );
        // All clones refer to this exact owned child. Reap before an identity
        // directory can be replaced, even if another request still owns a clone.
        if let Ok(mut child) = self.0.child.lock() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }
    fn request_timeout(
        &self,
        command: Value,
        write_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<Value, String> {
        let group_operation = command["op"]
            .as_str()
            .is_some_and(|operation| operation.starts_with("group_"));
        let event_send = command["op"] == "app_event_send";
        if self
            .0
            .child
            .lock()
            .map_err(|_| "联机组件状态不可用")?
            .try_wait()
            .map_err(|_| "联机组件状态不可用")?
            .is_some()
        {
            return Err("联机组件已退出".into());
        }
        let id = self
            .0
            .next_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| "联机会话序号已用尽")?;
        let mut bytes = serde_json::to_vec(&json!({"version":1,"id":id,"command":command}))
            .map_err(|_| "联机请求无效")?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err("联机请求过大".into());
        }
        let (receipt_tx, receipt_rx) = mpsc::channel();
        let (response_tx, response_rx) = mpsc::channel();
        {
            let mut pending = self.0.pending.lock().map_err(|_| "联机组件状态不可用")?;
            if pending.len() >= 64 {
                return Err("MESH_APP_BUSY".into());
            }
            pending.insert(id, response_tx);
        }
        struct Waiting(Pending, u64);
        impl Drop for Waiting {
            fn drop(&mut self) {
                if let Ok(mut pending) = self.0.lock() {
                    pending.remove(&self.1);
                }
            }
        }
        let _waiting = Waiting(self.0.pending.clone(), id);
        self.0
            .input
            .as_ref()
            .ok_or("联机组件已关闭")?
            .try_send((bytes, receipt_tx))
            .map_err(|_| "MESH_APP_BUSY")?;
        if receipt_rx.recv_timeout(write_timeout) != Ok(true) {
            return Err(if event_send {
                "MESH_EVENT_DELIVERY_UNKNOWN"
            } else if group_operation {
                "MESH_GROUP_IO_FAILED"
            } else {
                "联机组件未响应"
            }
            .into());
        }
        let response = response_rx
            .recv_timeout(read_timeout)
            .map_err(|_| {
                if event_send {
                    "MESH_EVENT_DELIVERY_UNKNOWN"
                } else if group_operation {
                    "MESH_GROUP_IO_FAILED"
                } else {
                    "联机连接超时"
                }
            })?
            .map_err(|_| {
                if event_send {
                    "MESH_EVENT_DELIVERY_UNKNOWN"
                } else if group_operation {
                    "MESH_GROUP_IO_FAILED"
                } else {
                    "联机组件返回无效数据"
                }
            })?;
        decode_response(response, id).map_err(|error| {
            if event_send && !error.starts_with("MESH_EVENT_") {
                "MESH_EVENT_DELIVERY_UNKNOWN".into()
            } else if group_operation && error == "联机组件返回无效数据" {
                "MESH_GROUP_IO_FAILED".into()
            } else if !group_operation && error.starts_with("MESH_GROUP_") {
                "联机连接失败，请检查中继或邀请后重试".into()
            } else {
                error
            }
        })
    }
}

fn decode_response(response: Value, id: u64) -> Result<Value, String> {
    if response.get("version").and_then(Value::as_u64) != Some(1)
        || response.get("id").and_then(Value::as_u64) != Some(id)
    {
        return Err("联机组件响应不匹配".into());
    }
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) => response
            .get("result")
            // Read-only identity lookup returns null before the first identity is created.
            .filter(|value| value.is_object() || value.is_null())
            .cloned()
            .ok_or_else(|| "联机组件返回无效数据".into()),
        Some(false) => {
            let code = response
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or("");
            if matches!(
                code,
                "event_namespace_invalid"
                    | "event_reserved_namespace"
                    | "event_name_invalid"
                    | "event_payload_invalid"
                    | "event_subscription_invalid"
                    | "event_subscription_unknown"
                    | "event_subscription_limit"
                    | "event_namespace_mismatch"
                    | "event_no_listener"
                    | "event_queue_full"
                    | "event_offline"
                    | "event_delivery_unknown"
                    | "event_peer_unknown"
                    | "event_cursor_invalid"
                    | "event_page_invalid"
                    | "event_busy"
                    | "event_message_invalid"
                    | "event_protocol_unsupported"
                    | "event_sequence_exhausted"
                    | "event_route_game_active"
                    | "event_route_active"
                    | "event_route_not_authorized"
                    | "event_direct_required"
                    | "event_route_failed"
                    | "event_route_not_ready"
            ) {
                return Err(format!("MESH_{}", code.to_ascii_uppercase()));
            }
            if code.starts_with("environment_")
                && code.len() <= 80
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            {
                return Err(format!("MESH_{}", code.to_ascii_uppercase()));
            }
            Err(match code {
                "direct_path_required" => "MESH_DIRECT_PATH_REQUIRED",
                "official_coordination_only" => "MESH_OFFICIAL_COORDINATION_ONLY",
                "invalid_route_policy" => "MESH_INVALID_ROUTE_POLICY",
                "invalid_display_name" => "MESH_INVALID_NAME",
                "invalid_contact_invitation" | "invalid_remote_profile" | "invalid_contact" => {
                    "MESH_INVALID_INVITATION"
                }
                "unknown_contact" | "contact_removed" => "MESH_UNKNOWN_CONTACT",
                "invalid_message" => "MESH_INVALID_MESSAGE",
                "app_busy" => "MESH_APP_BUSY",
                "app_stopped" | "app_not_initialized" | "app_not_listening" => "MESH_APP_STOPPED",
                "app_timeout"
                | "contact_no_addresses"
                | "wrong_peer"
                | "connection_denied"
                | "address_in_use"
                | "address_unavailable"
                | "connection_refused"
                | "contact_rejected" => "MESH_CONTACT_UNAVAILABLE",
                "contact_storage_failed" => "MESH_STORAGE_UNAVAILABLE",
                "session_active" | "operation_in_progress" => "已有联机会话正在进行",
                "invalid_invitation" | "invitation_failed" => "邀请无效或已过期",
                "invalid_relay_endpoint" | "unsupported_transport" => "中继地址无效或不支持",
                "transport_timeout" => "联机连接超时",
                "cancelled" => "联机操作已取消",
                "unknown_session" => "联机会话已结束",
                "invalid_approved_port" => "MESH_GROUP_LAN_NOT_OPEN",
                "identity_not_initialized" => "MESH_GROUP_IDENTITY_UNAVAILABLE",
                "unknown_group" | "group_ended" => "MESH_GROUP_NOT_ACTIVE",
                "not_group_host" => "MESH_GROUP_NOT_HOST",
                "invalid_peer" | "unknown_member" => "MESH_GROUP_UNKNOWN_MEMBER",
                "transport_failed" => "MESH_GROUP_JOIN_DENIED",
                _ => "联机连接失败，请检查中继或邀请后重试",
            }
            .into())
        }
        _ => Err("联机组件返回无效数据".into()),
    }
}

fn dispatch(frame: Value, pending: &Pending, events: &Events) {
    if frame.get("version").and_then(Value::as_u64) != Some(1) {
        return;
    }
    if matches!(
        frame.get("event").and_then(Value::as_str),
        Some("network_state_changed" | "group_state_changed")
    ) && frame.get("id").is_none()
    {
        let handler = events.lock().ok().and_then(|events| {
            if frame["event"] == "group_state_changed" {
                events.group.clone()
            } else {
                events.network.clone()
            }
        });
        if let Some(handler) = handler {
            handler(frame);
        }
    } else if let Some(id) = frame.get("id").and_then(Value::as_u64)
        && let Ok(mut pending) = pending.lock()
        && let Some(sender) = pending.remove(&id)
    {
        let _ = sender.send(Ok(frame));
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        self.input.take();
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn events_and_out_of_order_responses_are_demultiplexed_by_id() {
        let pending: Pending = Default::default();
        let events: Events = Default::default();
        let (a, first) = mpsc::channel();
        let (b, second) = mpsc::channel();
        pending.lock().unwrap().insert(1, a);
        pending.lock().unwrap().insert(2, b);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let captured = seen.clone();
        events.lock().unwrap().network =
            Some(Arc::new(move |event| captured.lock().unwrap().push(event)));
        let group_seen = Arc::new(Mutex::new(Vec::new()));
        let group_captured = group_seen.clone();
        events.lock().unwrap().group = Some(Arc::new(move |event| {
            group_captured.lock().unwrap().push(event)
        }));
        dispatch(
            json!({"version":1,"event":"group_state_changed","generation":7,"group":{"group_id":9}}),
            &pending,
            &events,
        );
        dispatch(
            json!({"version":1,"event":"network_state_changed","generation":4,"snapshot":{"phase":"online"}}),
            &pending,
            &events,
        );
        dispatch(
            json!({"version":1,"id":2,"ok":true,"result":{"second":true}}),
            &pending,
            &events,
        );
        dispatch(
            json!({"version":1,"id":1,"ok":true,"result":{"first":true}}),
            &pending,
            &events,
        );
        assert_eq!(seen.lock().unwrap().len(), 1);
        assert_eq!(group_seen.lock().unwrap().len(), 1);
        assert_eq!(group_seen.lock().unwrap()[0]["generation"], 7);
        assert_eq!(
            decode_response(second.recv().unwrap().unwrap(), 2).unwrap()["second"],
            true
        );
        assert_eq!(
            decode_response(first.recv().unwrap().unwrap(), 1).unwrap()["first"],
            true
        );
        assert!(pending.lock().unwrap().is_empty());
    }
    #[test]
    fn owned_blocked_companion_shutdown_is_bounded_and_aborts_pending_io() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("blocked.rs");
        let executable = root.path().join(if cfg!(windows) {
            "blocked.exe"
        } else {
            "blocked"
        });
        std::fs::write(&source, r#"use std::io::{BufRead,Write}; fn main(){let mut line=String::new(); std::io::stdin().lock().read_line(&mut line).unwrap(); println!("{{\"version\":1,\"event\":\"network_state_changed\",\"generation\":1,\"snapshot\":{{\"generation\":1}}}}"); std::io::stdout().flush().unwrap(); std::thread::sleep(std::time::Duration::from_secs(60));}"#).unwrap();
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compiled = Command::new(compiler)
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "owned protocol fixture did not compile"
        );
        let client = Client::spawn(&executable).unwrap();
        let (seen, event) = mpsc::channel();
        client.on_event(Arc::new(move |_| {
            let _ = seen.send(());
        }));
        let pending = client.clone();
        let request = thread::spawn(move || pending.request(json!({"op":"blocked"})));
        event.recv_timeout(Duration::from_secs(3)).unwrap();
        let started = std::time::Instant::now();
        client.shutdown();
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(!client.alive());
        assert!(request.join().unwrap().is_err());
        drop(client);
    }
    #[test]
    #[ignore = "Explicit owned new-protocol companion required; briefly connects pinned official Mesh services"]
    fn actual_companion_network_events_retry_and_stop_preserve_response_identity() {
        let helper =
            std::env::var_os("COBBLE_MESH_TEST_HELPER").expect("explicit owned helper path");
        let directory = tempfile::tempdir().unwrap();
        let client = Client::spawn(Path::new(&helper)).unwrap();
        let (events, received) = mpsc::channel();
        client.on_event(Arc::new(move |event| {
            let _ = events.send(event);
        }));
        let initial = client
            .request(json!({"op":"network_start","directory":directory.path().join("identity")}))
            .unwrap();
        let initial_generation = initial["generation"].as_u64().unwrap();
        assert!(initial_generation > 0);
        let public = client.request(json!({"op":"app_status"})).unwrap();
        assert!(public["profile"]["peer_id"].is_string());
        let mut typed_snapshot = initial.clone();
        typed_snapshot["enabled"] = json!(true);
        let typed_snapshot: super::super::network::MeshNetworkSnapshot =
            serde_json::from_value(typed_snapshot).unwrap();
        assert_eq!(
            typed_snapshot.profile.unwrap().peer_id,
            public["profile"]["peer_id"].as_str().unwrap()
        );
        assert_eq!(typed_snapshot.checks.len(), 3);
        let retried = client.request(json!({"op":"network_retry"})).unwrap();
        assert!(retried["generation"].as_u64().unwrap() > initial_generation);
        let stopped = client.request(json!({"op":"network_stop"})).unwrap();
        let stopped_generation = stopped["generation"].as_u64().unwrap();
        assert_eq!(stopped["phase"], "offline");
        assert!(stopped_generation > retried["generation"].as_u64().unwrap());
        let mut saw_event = false;
        let deadline = std::time::Instant::now() + Duration::from_millis(250);
        while let Ok(event) =
            received.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
        {
            if event["event"] == "network_state_changed" {
                saw_event = true;
                assert!(event.get("id").is_none());
                if event["generation"].as_u64().unwrap() >= stopped_generation {
                    assert_eq!(event["snapshot"]["phase"], "offline");
                }
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
        }
        assert!(saw_event);
        client.shutdown();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while client.alive() {
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn absent_identity_is_a_successful_null_result() {
        assert_eq!(
            decode_response(json!({"version":1,"id":1,"ok":true,"result":null}), 1),
            Ok(Value::Null)
        );
        assert!(decode_response(json!({"version":1,"id":1,"ok":true}), 1).is_err());
        assert!(decode_response(json!({"version":1,"id":1,"ok":true,"result":[]}), 1).is_err());
    }

    #[test]
    #[ignore = "Requires explicit owned companion; read-only lookup in disposable directory, no network"]
    fn actual_companion_absent_identity_is_not_an_error() {
        let helper = std::env::var_os("COBBLE_MESH_TEST_HELPER").expect("explicit helper path");
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("not-created");
        let client = Client::spawn(Path::new(&helper)).unwrap();
        let result = client.request(json!({"op":"app_identity_profile","directory":directory}));
        client.shutdown();
        drop(client);
        assert_eq!(result, Ok(Value::Null));
        assert!(!directory.exists());
    }

    #[test]
    #[ignore = "Requires explicit developer-only companion path; performs no network operations"]
    fn actual_companion_status_end_shutdown_reaps_owned_process() {
        let path = std::env::var_os("COBBLE_MESH_TEST_HELPER")
            .expect("test helper path must be provided explicitly");
        let client = Client::spawn(Path::new(&path)).unwrap();
        assert!(client.request(json!({"op":"status"})).unwrap()["session"].is_null());
        assert!(
            client
                .request(json!({"op":"end","session_id":123}))
                .is_err()
        );
        assert!(client.request(json!({"op":"status"})).unwrap()["session"].is_null());
        assert_eq!(
            client.request(json!({"op":"shutdown"})).unwrap()["state"],
            "shutdown"
        );
        let started = std::time::Instant::now();
        while client.0.child.lock().unwrap().try_wait().unwrap().is_none() {
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "owned helper did not exit after shutdown"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn frames_are_bounded_and_secret_errors_are_not_forwarded() {
        assert!(read_frame(&mut std::io::Cursor::new(vec![b'x'; MAX_FRAME + 1])).is_err());
        assert!(read_frame(&mut std::io::Cursor::new(b"{}".to_vec())).is_err());
        assert!(read_frame(&mut std::io::Cursor::new(b"{}\n".to_vec())).is_ok());
        let secret = "private-invitation-value";
        let error = decode_response(
            json!({"version":1,"id":1,"ok":false,"error":{"code":secret,"message":secret}}),
            1,
        )
        .unwrap_err();
        assert!(!error.contains(secret));
        assert!(decode_response(json!({"version":1,"id":2,"ok":true,"result":{}}), 1).is_err());
    }
    #[test]
    fn mesh_event_error_codes_are_closed_and_delivery_uncertainty_is_preserved() {
        for code in [
            "event_no_listener",
            "event_queue_full",
            "event_delivery_unknown",
            "event_subscription_unknown",
            "event_protocol_unsupported",
        ] {
            assert_eq!(decode_response(json!({"version":1,"id":1,"ok":false,"error":{"code":code,"message":"PRIVATE peer and payload"}}),1).unwrap_err(),format!("MESH_{}",code.to_ascii_uppercase()));
        }
        let unknown=decode_response(json!({"version":1,"id":1,"ok":false,"error":{"code":"event_private_payload_secret","message":"PRIVATE"}}),1).unwrap_err();
        assert!(!unknown.contains("PRIVATE") && !unknown.contains("SECRET"));
    }
}

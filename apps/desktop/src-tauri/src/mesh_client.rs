// SPDX-License-Identifier: MIT
//! Owned companion process and bounded, private stdio frames. Never log protocol data.
use process_wrap::std::{ChildWrapper, CommandWrap};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

pub(super) const MAX_FRAME: usize = 8192;
pub(super) struct Client {
    child: Box<dyn ChildWrapper>,
    input: Option<mpsc::Sender<(Vec<u8>, mpsc::Sender<bool>)>>,
    output: mpsc::Receiver<Result<Value, ()>>,
    workers: Vec<thread::JoinHandle<()>>,
    next_id: u64,
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
        let (write_tx, write_rx) = mpsc::channel::<(Vec<u8>, mpsc::Sender<bool>)>();
        let writer = thread::spawn(move || {
            for (bytes, receipt) in write_rx {
                let ok = input.write_all(&bytes).and_then(|()| input.flush()).is_ok();
                let _ = receipt.send(ok);
                if !ok {
                    break;
                }
            }
        });
        let (read_tx, read_rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut output = BufReader::new(output);
            loop {
                let frame = read_frame(&mut output);
                let failed = frame.is_err();
                if read_tx.send(frame).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: Some(write_tx),
            output: read_rx,
            workers: vec![writer, reader],
            next_id: 1,
        })
    }

    pub(super) fn request(&mut self, command: Value) -> Result<Value, String> {
        self.request_timeout(command, Duration::from_secs(5), Duration::from_secs(20))
    }
    pub(super) fn shutdown(&mut self) {
        let _ = self.request_timeout(
            json!({"op":"shutdown"}),
            Duration::from_secs(1),
            Duration::from_secs(3),
        );
    }
    fn request_timeout(
        &mut self,
        command: Value,
        write_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<Value, String> {
        if self
            .child
            .try_wait()
            .map_err(|_| "联机组件状态不可用")?
            .is_some()
        {
            return Err("联机组件已退出".into());
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or("联机会话序号已用尽")?;
        let mut bytes = serde_json::to_vec(&json!({"version":1,"id":id,"command":command}))
            .map_err(|_| "联机请求无效")?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME {
            return Err("联机请求过大".into());
        }
        let (receipt_tx, receipt_rx) = mpsc::channel();
        self.input
            .as_ref()
            .ok_or("联机组件已关闭")?
            .send((bytes, receipt_tx))
            .map_err(|_| "联机组件已关闭")?;
        if receipt_rx.recv_timeout(write_timeout) != Ok(true) {
            return Err("联机组件未响应".into());
        }
        let response = self
            .output
            .recv_timeout(read_timeout)
            .map_err(|_| "联机连接超时")?
            .map_err(|_| "联机组件返回无效数据")?;
        decode_response(response, id)
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
            Err(match code {
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
                _ => "联机连接失败，请检查中继或邀请后重试",
            }
            .into())
        }
        _ => Err("联机组件返回无效数据".into()),
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut client = Client::spawn(Path::new(&helper)).unwrap();
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
        let mut client = Client::spawn(Path::new(&path)).unwrap();
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
        while client.child.try_wait().unwrap().is_none() {
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
}

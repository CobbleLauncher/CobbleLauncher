// SPDX-License-Identifier: MIT
//! Shared bounded pipe transport for an owned isolated plugin worker.
use cobble_plugin_api::PluginError;
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(crate) struct WorkerSession {
    child: super::process::OwnedChild,
    requests: Option<mpsc::SyncSender<Vec<u8>>>,
    replies: Option<mpsc::Receiver<Result<Vec<u8>, PluginError>>>,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
}
pub(crate) fn frames(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(frames.iter().map(|frame| frame.len() + 4).sum());
    for frame in frames {
        bytes.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        bytes.extend_from_slice(frame);
    }
    bytes
}
impl WorkerSession {
    pub(crate) fn spawn(
        host: &Path,
        mode: &str,
        initial: Vec<Vec<u8>>,
        maximum_reply: usize,
        cancel: &AtomicBool,
    ) -> Result<Self, PluginError> {
        if cancel.load(Ordering::Acquire) {
            return Err(cancelled());
        }
        let mut command = Command::new(host);
        command
            .arg(mode)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = super::process::spawn(command)
            .map_err(|_| PluginError::new("PLUGIN_IO", "Could not start plugin worker"))?;
        let mut input = child.stdin().take().expect("piped stdin");
        let mut output = child.stdout().take().expect("piped stdout");
        let (requests, rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let writer = std::thread::spawn(move || {
            for frame in initial {
                if input
                    .write_all(&(frame.len() as u32).to_le_bytes())
                    .and_then(|()| input.write_all(&frame))
                    .is_err()
                {
                    return;
                }
            }
            if input.flush().is_err() {
                return;
            }
            for request in rx {
                if input
                    .write_all(&request)
                    .and_then(|()| input.flush())
                    .is_err()
                {
                    return;
                }
            }
        });
        let (tx, replies) = mpsc::sync_channel(1);
        let reader = std::thread::spawn(move || {
            loop {
                let result = (|| {
                    let mut length = [0; 4];
                    output.read_exact(&mut length).map_err(|_| {
                        PluginError::new("PLUGIN_CRASH", "Plugin worker closed reply pipe")
                    })?;
                    let count = u32::from_le_bytes(length) as usize;
                    if count > maximum_reply {
                        return Err(PluginError::new(
                            "PLUGIN_LIMIT",
                            "Plugin reply exceeds budget",
                        ));
                    }
                    let mut bytes = vec![0; count];
                    output.read_exact(&mut bytes).map_err(|_| {
                        PluginError::new("PLUGIN_PROTOCOL", "Truncated plugin reply")
                    })?;
                    Ok(bytes)
                })();
                let failed = result.is_err();
                if tx.send(result).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            requests: Some(requests),
            replies: Some(replies),
            writer: Some(writer),
            reader: Some(reader),
        })
    }
    pub(crate) fn finish_input(&mut self) {
        self.requests.take();
    }
    pub(crate) fn send(&mut self, bytes: Vec<u8>) -> Result<(), PluginError> {
        if bytes.len() > cobble_plugin_api::MAX_FILE_BYTES + 16384 {
            return Err(PluginError::new(
                "PLUGIN_LIMIT",
                "Plugin request exceeds budget",
            ));
        }
        let sent = self
            .requests
            .as_ref()
            .ok_or_else(|| PluginError::new("PLUGIN_CLOSED", "Plugin session is closed"))?
            .send(bytes);
        if sent.is_err() {
            self.close();
            return Err(PluginError::new(
                "PLUGIN_CRASH",
                "Plugin request pipe closed",
            ));
        }
        Ok(())
    }
    pub(crate) fn receive<T: serde::de::DeserializeOwned>(
        &mut self,
        cancel: &AtomicBool,
    ) -> Result<T, PluginError> {
        self.receive_validated(cancel, Ok)
    }
    pub(crate) fn receive_validated<T: serde::de::DeserializeOwned>(
        &mut self,
        cancel: &AtomicBool,
        validate: impl FnOnce(T) -> Result<T, PluginError>,
    ) -> Result<T, PluginError> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let result = loop {
            if cancel.load(Ordering::Acquire) {
                break Err(cancelled());
            }
            if Instant::now() >= deadline {
                break Err(PluginError::new(
                    "PLUGIN_TIMEOUT",
                    "Plugin request exceeded 10 seconds",
                ));
            }
            match self
                .replies
                .as_ref()
                .ok_or_else(|| PluginError::new("PLUGIN_CLOSED", "Plugin session is closed"))?
                .recv_timeout(Duration::from_millis(10))
            {
                Ok(Ok(bytes)) => {
                    break match serde_json::from_slice::<Result<T, PluginError>>(&bytes) {
                        Ok(result) => result,
                        Err(_) => Err(PluginError::new(
                            "PLUGIN_INVALID_REPLY",
                            "Invalid plugin reply",
                        )),
                    };
                }
                Ok(Err(error)) => break Err(error),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Err(PluginError::new("PLUGIN_CRASH", "Plugin worker exited"));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        .and_then(validate);
        if result.is_err() {
            self.close();
            return result;
        }
        if cancel.load(Ordering::Acquire) {
            self.close();
            return Err(cancelled());
        }
        if Instant::now() >= deadline {
            self.close();
            return Err(PluginError::new(
                "PLUGIN_TIMEOUT",
                "Plugin request exceeded 10 seconds",
            ));
        }
        result
    }
    pub(crate) fn close(&mut self) {
        // A one-shot sender may already have been closed to signal EOF.
        self.requests.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        self.replies.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
    pub(crate) fn process_id(&self) -> u32 {
        self.child.id()
    }
    pub(crate) fn has_exited(&mut self) -> std::io::Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }
}
fn cancelled() -> PluginError {
    PluginError::new("CANCEL_REQUESTED", "Plugin call cancelled")
}
impl Drop for WorkerSession {
    fn drop(&mut self) {
        self.close();
    }
}

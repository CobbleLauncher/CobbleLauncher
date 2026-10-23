// SPDX-License-Identifier: MIT
use cobble_plugin_api::{
    MAX_COMPONENT_BYTES, MAX_FILE_BYTES, MAX_REPLY_BYTES, PluginError, SceneChunk, ViewerScene,
};
use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};
pub struct PluginSession {
    worker: super::worker_session::WorkerSession,
    summary: ViewerScene,
}
impl PluginSession {
    pub fn open(
        host: &Path,
        component: &[u8],
        data: &[u8],
        cancel: Arc<AtomicBool>,
    ) -> Result<Self, PluginError> {
        if component.len() > MAX_COMPONENT_BYTES || data.len() > MAX_FILE_BYTES {
            return Err(PluginError::new(
                "PLUGIN_LIMIT",
                "Input exceeds Viewer budget",
            ));
        }
        let mut worker = super::worker_session::WorkerSession::spawn(
            host,
            "--viewer-stdio",
            vec![component.to_vec(), data.to_vec()],
            MAX_REPLY_BYTES,
            &cancel,
        )?;
        let summary = worker.receive_validated(&cancel, |summary: ViewerScene| {
            summary.validate()?;
            Ok(summary)
        })?;
        Ok(Self { worker, summary })
    }
    pub fn summary(&self) -> &ViewerScene {
        &self.summary
    }
    pub fn read_chunk(
        &mut self,
        index: u32,
        cancel: Arc<AtomicBool>,
    ) -> Result<SceneChunk, PluginError> {
        if index >= self.summary.chunk_count {
            return Err(PluginError::new(
                "VIEWER_INVALID_CHUNK",
                "Chunk index is outside this scene",
            ));
        }
        self.worker.send(index.to_le_bytes().to_vec())?;
        self.worker.receive_validated(&cancel, |chunk: SceneChunk| {
            self.summary.validate_chunk(&chunk)?;
            Ok(chunk)
        })
    }
    pub fn close(&mut self) {
        self.worker.close();
    }
    pub fn process_id(&self) -> u32 {
        self.worker.process_id()
    }
    pub fn has_exited(&mut self) -> std::io::Result<bool> {
        self.worker.has_exited()
    }
}

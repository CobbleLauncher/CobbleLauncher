// SPDX-License-Identifier: MIT
//! Delegates storage to rustic's local backend; adds cooperative cancellation at I/O.
use crate::BackupControl;
use bytes::Bytes;
use rustic_backend::LocalBackend;
use rustic_core::{
    BytesList, ErrorKind, FileType, Id, ReadBackend, RusticError, RusticResult, WriteBackend,
};

#[derive(Debug)]
pub(crate) struct CheckedBackend {
    pub(crate) local: LocalBackend,
    pub(crate) control: BackupControl,
}
impl CheckedBackend {
    fn check(&self) -> RusticResult<()> {
        if self.control.is_cancelled() {
            Err(RusticError::new(
                ErrorKind::InputOutput,
                "Backup operation cancelled",
            ))
        } else {
            Ok(())
        }
    }
}
impl ReadBackend for CheckedBackend {
    fn location(&self) -> String {
        self.local.location()
    }
    fn list_with_size(&self, kind: FileType) -> RusticResult<Vec<(Id, u32)>> {
        self.check()?;
        self.local.list_with_size(kind)
    }
    fn read_full(&self, kind: FileType, id: &Id) -> RusticResult<Bytes> {
        self.check()?;
        self.local.read_full(kind, id)
    }
    fn read_partial(
        &self,
        kind: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        self.check()?;
        self.local.read_partial(kind, id, cacheable, offset, length)
    }
    fn warmup_path(&self, kind: FileType, id: &Id) -> String {
        self.local.warmup_path(kind, id)
    }
}
impl WriteBackend for CheckedBackend {
    fn create(&self) -> RusticResult<()> {
        self.check()?;
        self.local.create()
    }
    fn write_bytes(
        &self,
        kind: FileType,
        id: &Id,
        cacheable: bool,
        content: BytesList,
    ) -> RusticResult<()> {
        self.check()?;
        self.local.write_bytes(kind, id, cacheable, content)
    }
    fn remove(&self, kind: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        self.check()?;
        self.local.remove(kind, id, cacheable)
    }
}

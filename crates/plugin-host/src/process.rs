// SPDX-License-Identifier: MIT
//! The application owns the worker and, on Windows, its kill-on-close job.
use process_wrap::std::{ChildWrapper, CommandWrap};
pub(super) type OwnedChild = Box<dyn ChildWrapper>;
pub(super) fn spawn(command: std::process::Command) -> std::io::Result<OwnedChild> {
    let mut command = CommandWrap::from(command);
    #[cfg(windows)]
    {
        use process_wrap::std::{CreationFlags, JobObject};
        let mut flags = CreationFlags(Default::default());
        flags.0.0 = 0x0800_0000;
        command.wrap(flags).wrap(JobObject);
    }
    command.spawn()
}

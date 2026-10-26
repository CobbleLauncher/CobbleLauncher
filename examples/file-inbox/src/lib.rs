// SPDX-License-Identifier: MIT
//! Native-backed file delivery frontend. Direct report input is unsupported.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::Report;
use launcher::resource_report::resources::{Failure, Input};
struct FileInbox;
impl exports::launcher::resource_report::reports::Guest for FileInbox {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        Err(Failure::Unsupported)
    }
}
export!(FileInbox);

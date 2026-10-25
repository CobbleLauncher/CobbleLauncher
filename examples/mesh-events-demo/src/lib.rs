// SPDX-License-Identifier: MIT
//! The report guest supplies a fallback. Mesh communication belongs to the scoped web SDK.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct ReadyCheck;
impl Guest for ReadyCheck {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(Report { schema_version: 1, findings: vec![Finding {
            code: "mesh_ready_check".into(), severity: Severity::Info,
            title: "Ready check".into(),
            message: "Open the custom view to exchange readiness with a Mesh friend.".into(),
            evidence: vec![], suggestions: vec![],
        }], tables: vec![] })
    }
}
export!(ReadyCheck);

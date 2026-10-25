// SPDX-License-Identifier: MIT
//! The assistant runs through host-authorized frontend methods. This report
//! component is a valid, inert entry point for the optional plugin package.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin"});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};

struct AiAssistant;
impl Guest for AiAssistant {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(Report {
            schema_version: 1,
            findings: vec![Finding {
                code: "ai_assistant_frontend".into(),
                severity: Severity::Info,
                title: "AI Assistant".into(),
                message: "Open AI Assistant from Tools.".into(),
                evidence: vec![],
                suggestions: vec![],
            }],
            tables: vec![],
        })
    }
}
export!(AiAssistant);

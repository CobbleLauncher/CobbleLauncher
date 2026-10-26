// SPDX-License-Identifier: MIT
//! The independent-copy workflow lives in the host-authorized custom frontend.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct TrialCopy;
impl Guest for TrialCopy {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report { schema_version: 1, findings: vec![Finding {
            code: "trial_copy_frontend".into(), severity: Severity::Info,
            title: if zh { "更新试验副本" } else { "Update Trial Copy" }.into(),
            message: if zh { "请从工具页面打开更新试验副本，创建独立副本以试验更新。" } else { "Open Update Trial Copy from Tools to create an independent copy to try updates." }.into(),
            evidence: vec![], suggestions: vec![],
        }], tables: vec![] })
    }
}
export!(TrialCopy);

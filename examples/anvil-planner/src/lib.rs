// SPDX-License-Identifier: MIT
//! Offline arithmetic is performed cooperatively by the bundled frontend.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct AnvilPlanner;
impl Guest for AnvilPlanner {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() { return Err(Failure::Cancelled); }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report { schema_version: 1, findings: vec![Finding {
            code: "anvil_planner_frontend".into(), severity: Severity::Info,
            title: if zh { "铁砧规划" } else { "Anvil Planner" }.into(),
            message: if zh { "打开铁砧规划，输入装备、附魔书和已有惩罚。" } else { "Open Anvil Planner and enter equipment, books and prior work." }.into(),
            evidence: vec![], suggestions: vec![],
        }], tables: vec![] })
    }
}
export!(AnvilPlanner);

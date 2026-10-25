// SPDX-License-Identifier: MIT
//! The material planning frontend uses host-provided counts, never a blueprint parser.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct MaterialPlanner;
impl Guest for MaterialPlanner {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() { return Err(Failure::Cancelled); }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report { schema_version: 1, findings: vec![Finding {
            code: "material_planner_frontend".into(), severity: Severity::Info,
            title: if zh { "材料筹备清单" } else { "Material Planner" }.into(),
            message: if zh { "从蓝图材料页面发送统计，再在材料筹备清单中记录收集进度。" } else { "Send material counts from a blueprint, then record collection progress in Material Planner." }.into(),
            evidence: vec![], suggestions: vec![],
        }], tables: vec![] })
    }
}
export!(MaterialPlanner);

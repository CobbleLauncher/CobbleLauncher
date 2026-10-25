// SPDX-License-Identifier: MIT
//! The saved-location editor lives in the host-authorized custom frontend.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct CoordinateBook;
impl Guest for CoordinateBook {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() { return Err(Failure::Cancelled); }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report { schema_version: 1, findings: vec![Finding {
            code: "coordinate_book_frontend".into(), severity: Severity::Info,
            title: if zh { "坐标簿" } else { "Coordinate Book" }.into(),
            message: if zh { "请从工具页面打开坐标簿，查看和编辑已保存的坐标。" } else { "Open Coordinate Book from Tools to view and edit saved locations." }.into(),
            evidence: vec![], suggestions: vec![],
        }], tables: vec![] })
    }
}
export!(CoordinateBook);

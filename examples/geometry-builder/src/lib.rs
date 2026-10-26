// SPDX-License-Identifier: MIT
//! Offline shape calculation lives in the web frontend; native writes remain host-owned.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct GeometryBuilder;
impl Guest for GeometryBuilder {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report {
            schema_version: 1,
            findings: vec![Finding {
                code: "geometry_builder_frontend".into(),
                severity: Severity::Info,
                title: if zh {
                    "几何建筑"
                } else {
                    "Geometry Builder"
                }
                .into(),
                message: if zh {
                    "打开几何建筑以生成分层形状。"
                } else {
                    "Open Geometry Builder to create a layered shape."
                }
                .into(),
                evidence: vec![],
                suggestions: vec![],
            }],
            tables: vec![],
        })
    }
}
export!(GeometryBuilder);

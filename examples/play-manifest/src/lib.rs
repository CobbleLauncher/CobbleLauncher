// SPDX-License-Identifier: MIT
//! The saved-location editor lives in the host-authorized custom frontend.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
struct PlayManifest;
impl Guest for PlayManifest {
    fn analyze(_input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let zh = launcher::report_context::context::get().locale == "zh-CN";
        Ok(Report {
            schema_version: 1,
            findings: vec![Finding {
                code: "play_manifest_frontend".into(),
                severity: Severity::Info,
                title: if zh {
                    "开玩配置"
                } else {
                    "Play Configuration"
                }
                .into(),
                message: if zh {
                    "请从工具页面打开开玩配置，预览并导出游戏配置。"
                } else {
                    "Open Play Configuration from Tools to preview and export game configurations."
                }
                .into(),
                evidence: vec![],
                suggestions: vec![],
            }],
            tables: vec![],
        })
    }
}
export!(PlayManifest);

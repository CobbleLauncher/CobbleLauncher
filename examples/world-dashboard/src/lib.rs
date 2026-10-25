// SPDX-License-Identifier: MIT
//! Original read-only analysis of the launcher's bounded world metadata snapshot.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_ROWS: usize = 128;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn text(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("—")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect()
}
fn note(report: &mut Report, code: &str, severity: Severity, message: String, zh: bool) {
    if report.findings.len() < 32 {
        report.findings.push(Finding {
            code: code.into(),
            severity,
            title: tr(zh, "World check", "世界检查").into(),
            message,
            evidence: vec![],
            suggestions: vec![],
        });
    }
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if root["schemaVersion"] != 1
        || root["scope"] != "worlds"
        || !root["game"].is_object()
        || !root["revision"].is_string()
    {
        return Err(Failure::Unsupported);
    }
    let zh = locale == "zh-CN";
    let mut report = Report {
        schema_version: 1,
        findings: vec![],
        tables: vec![Table {
            columns: vec![
                tr(zh, "World", "世界").into(),
                tr(zh, "Recorded version", "记录版本").into(),
                tr(zh, "Mode", "模式").into(),
                tr(zh, "Last played", "上次游玩").into(),
            ],
            rows: vec![],
        }],
    };
    let Some(items) = root["data"]["items"]
        .as_array()
        .filter(|_| root["data"]["unavailable"].is_null())
    else {
        note(
            &mut report,
            "worlds_unavailable",
            Severity::Warning,
            tr(
                zh,
                "World inventory is unavailable. No world count was inferred.",
                "世界清单不可用，无法确认世界数量。",
            )
            .into(),
            zh,
        );
        return Ok(report);
    };
    let now = root["data"]["inspectedAt"].as_u64();
    let mut errors = 0;
    let mut hardcore = 0;
    let mut different = 0;
    let mut future_times = 0;
    for item in items.iter().take(MAX_ROWS) {
        if let (Some(inspected), Some(last_played)) = (now, item["lastPlayed"].as_u64()) {
            if last_played / 1000 > inspected {
                future_times += 1;
            }
        }
        let metadata_error = item["metadataError"].as_bool().unwrap_or(true);
        if metadata_error {
            errors += 1;
        }
        if item["hardcore"] == true {
            hardcore += 1;
        }
        if item["version"].is_string()
            && root["game"]["version"].is_string()
            && item["version"] != root["game"]["version"]
        {
            different += 1;
        }
        let mode = if metadata_error {
            tr(zh, "Unknown", "未知")
        } else {
            match item["gameMode"].as_u64() {
                Some(0) => tr(zh, "Survival", "生存"),
                Some(1) => tr(zh, "Creative", "创造"),
                Some(2) => tr(zh, "Adventure", "冒险"),
                Some(3) => tr(zh, "Spectator", "旁观"),
                _ => tr(zh, "Unknown", "未知"),
            }
        };
        let mode = if item["hardcore"] == true {
            format!("{mode} · {}", tr(zh, "Hardcore", "极限"))
        } else {
            mode.into()
        };
        let last = match (now, item["lastPlayed"].as_u64().filter(|n| *n > 0)) {
            (Some(now), Some(ms)) if ms / 1000 <= now => {
                let days = (now - ms / 1000) / 86400;
                if days == 0 {
                    tr(zh, "Within one day", "一天内").into()
                } else {
                    format!(
                        "{days} {}",
                        tr(zh, if days == 1 { "day ago" } else { "days ago" }, "天前")
                    )
                }
            }
            _ => tr(zh, "Unknown", "未知").into(),
        };
        report.tables[0].rows.push(vec![
            text(&item["name"]),
            text(&item["version"]),
            mode,
            last,
        ]);
    }
    let listed = report.tables[0].rows.len();
    note(
        &mut report,
        "worlds_summary",
        Severity::Info,
        format!(
            "{}: {}; {}: {}",
            tr(zh, "Worlds listed", "已列出世界"),
            listed,
            tr(zh, "Hardcore", "极限模式"),
            hardcore
        ),
        zh,
    );
    if items.is_empty() {
        note(
            &mut report,
            "worlds_empty",
            Severity::Info,
            tr(
                zh,
                "No worlds were found in this game's saves folder.",
                "此游戏的存档目录中未发现世界。",
            )
            .into(),
            zh,
        );
    }
    if errors > 0 {
        note(
            &mut report,
            "world_metadata_unreadable",
            Severity::Warning,
            format!(
                "{errors} {}",
                tr(
                    zh,
                    "worlds have unreadable metadata; mode and version may be unknown.",
                    "个世界的元数据无法读取，模式和版本可能未知。"
                )
            ),
            zh,
        );
    }
    if different > 0 {
        note(
            &mut report,
            "world_recorded_version_differs",
            Severity::Info,
            format!(
                "{different} {}",
                tr(
                    zh,
                    "worlds record a different version from this game. This is metadata, not a compatibility test.",
                    "个世界记录的版本与游戏不同；尚未验证兼容性。"
                )
            ),
            zh,
        );
    }
    if future_times > 0 {
        note(
            &mut report,
            "world_last_played_after_inspection",
            Severity::Info,
            format!(
                "{future_times} {}",
                tr(
                    zh,
                    "worlds have a last-played time after this check. Recent-play order is unknown.",
                    "个世界的上次游玩时间晚于本次检查，无法确定最近游玩顺序。"
                )
            ),
            zh,
        );
    }
    if root["data"]["truncated"] == true || items.len() > MAX_ROWS {
        note(
            &mut report,
            "world_inventory_partial",
            Severity::Warning,
            tr(
                zh,
                "Only part of the world inventory is shown.",
                "仅显示部分世界清单。",
            )
            .into(),
            zh,
        );
    }
    Ok(report)
}
struct WorldDashboard;
impl exports::launcher::resource_report::reports::Guest for WorldDashboard {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let metadata = input.describe()?;
        if metadata.media_type != "application/json" {
            return Err(Failure::Unsupported);
        }
        if metadata.byte_length > MAX_BYTES as u64 {
            return Err(Failure::Limit);
        }
        let mut bytes = Vec::with_capacity(metadata.byte_length as usize);
        while bytes.len() < metadata.byte_length as usize {
            if resources::cancelled() {
                return Err(Failure::Cancelled);
            }
            let maximum = (metadata.byte_length as usize - bytes.len()).min(65536) as u32;
            let chunk = input.read_at(bytes.len() as u64, maximum)?;
            if chunk.is_empty() || chunk.len() > maximum as usize {
                return Err(Failure::Io);
            }
            bytes.extend_from_slice(&chunk);
        }
        let report = analyze_bytes(&bytes, &launcher::report_context::context::get().locale)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(WorldDashboard);
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn snapshot(items: Value) -> Value {
        json!({"schemaVersion":1,"scope":"worlds","game":{"id":"a","name":"Game","version":"1.21.1"},"data":{"items":items,"inspectedAt":172800},"revision":"r"})
    }
    #[test]
    fn reports_actual_metadata_and_unknown_fields() {
        let root = snapshot(
            json!([{"id":"a","name":"World","version":"1.20.1","gameMode":1,"hardcore":true,"lastPlayed":86400000,"metadataError":false},{"id":"b","name":"Broken","metadataError":true}]),
        );
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "en").unwrap();
        assert_eq!(report.tables[0].rows[0][3], "1 day ago");
        assert_eq!(report.tables[0].rows[1][2], "Unknown");
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "world_metadata_unreadable")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "world_recorded_version_differs")
        );
    }
    #[test]
    fn limits_and_future_times_remain_honest() {
        let item = json!({"name":"中".repeat(1000),"lastPlayed":u64::MAX,"metadataError":false});
        let report = analyze_bytes(
            &serde_json::to_vec(&snapshot(json!(vec![item; 200]))).unwrap(),
            "zh-CN",
        )
        .unwrap();
        assert_eq!(report.tables[0].rows.len(), 128);
        assert_eq!(report.tables[0].rows[0][0].chars().count(), 160);
        assert_eq!(report.tables[0].rows[0][3], "未知");
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "world_inventory_partial")
        );
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
        assert!(matches!(
            analyze_bytes(b"{}", "en"),
            Err(Failure::Unsupported)
        ));
    }
    #[test]
    fn unavailable_is_not_empty_inventory() {
        let mut root = snapshot(Value::Null);
        root["data"]["unavailable"] = json!("WORLD_IO");
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "en").unwrap();
        assert_eq!(report.findings[0].code, "worlds_unavailable");
        assert!(!report.findings.iter().any(|f| f.code == "worlds_empty"));
    }
    #[test]
    fn future_world_metadata_never_claims_a_recent_play_order() {
        let root = snapshot(
            json!([{"id":"world","name":"World","lastPlayed":172801000,"metadataError":false}]),
        );
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "en").unwrap();
        assert_eq!(report.tables[0].rows[0][3], "Unknown");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "world_last_played_after_inspection")
        );
    }
}

// SPDX-License-Identifier: MIT
//! Read-only audit of datapack metadata already collected for each world.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
#[cfg(target_arch = "wasm32")]
use launcher::resource_report::resources;
use launcher::resource_report::resources::{Failure, Input};
use serde_json::Value;

const MAX_BYTES: usize = 1024 * 1024;
fn cancelled() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        resources::cancelled()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn finding(code: &str, severity: Severity, message: String, advice: &str, zh: bool) -> Finding {
    Finding {
        code: code.into(),
        severity,
        title: tr(zh, "Datapack metadata", "数据包元数据").into(),
        message,
        evidence: vec![],
        suggestions: vec![advice.into()],
    }
}
fn string<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let zh = locale == "zh-CN";
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || value.get("scope").and_then(Value::as_str) != Some("datapacks")
    {
        return Err(Failure::Unsupported);
    }
    let items = value
        .get("data")
        .and_then(|v| v.get("items"))
        .and_then(Value::as_array)
        .ok_or(Failure::Unsupported)?;
    let truncated = value["data"]["truncated"].as_bool().unwrap_or(false);
    let mut report = Report {
        schema_version: 1,
        findings: vec![],
        tables: vec![Table {
            columns: vec![
                tr(zh, "World", "世界").into(),
                tr(zh, "Datapack", "数据包").into(),
                tr(zh, "State", "状态").into(),
                tr(zh, "Pack format", "数据包格式").into(),
                tr(zh, "Size", "大小").into(),
            ],
            rows: vec![],
        }],
    };
    let mut identifiers = std::collections::BTreeSet::new();
    let mut missing_names = 0usize;
    let mut duplicates = 0usize;
    for item in items.iter().take(4096) {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let world = string(item, "worldId");
        let id = string(item, "id");
        let name = string(item, "name");
        let removed = item
            .get("removed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let known_format = item.get("packFormat").and_then(Value::as_u64);
        let pack_format = known_format
            .map(|v| v.to_string())
            .unwrap_or_else(|| tr(zh, "Unavailable", "不可用").into());
        let size = item
            .get("sizeBytes")
            .and_then(Value::as_u64)
            .map(|v| format!("{v} B"))
            .unwrap_or_else(|| tr(zh, "Unknown", "未知").into());
        report.tables[0].rows.push(vec![
            world.into(),
            if name.trim().is_empty() {
                id.into()
            } else {
                name.into()
            },
            tr(
                zh,
                if removed {
                    "Removed folder"
                } else {
                    "Installed"
                },
                if removed {
                    "已移除的文件夹"
                } else {
                    "已安装"
                },
            )
            .into(),
            pack_format,
            size,
        ]);
        if name.trim().is_empty() {
            missing_names += 1;
            report.findings.push(finding(
                "datapack_name_missing",
                Severity::Warning,
                format!(
                    "{} · {}",
                    if world.is_empty() {
                        tr(zh, "World", "世界")
                    } else {
                        world
                    },
                    if id.is_empty() {
                        tr(zh, "Identifier unavailable", "标识不可用")
                    } else {
                        id
                    }
                ),
                tr(
                    zh,
                    "Add a display name if this datapack's metadata supports one.",
                    "如果此数据包的元数据支持显示名称，请补充名称。",
                ),
                zh,
            ));
        }
        if id.is_empty() {
            report.findings.push(finding(
                "datapack_identifier_missing",
                Severity::Warning,
                if world.is_empty() {
                    tr(zh, "World", "世界").into()
                } else {
                    world.into()
                },
                tr(zh, "This datapack has no identifier.", "此数据包缺少标识。"),
                zh,
            ));
        }
        if !id.is_empty() && !identifiers.insert((world.to_owned(), id.to_owned())) {
            duplicates += 1;
            report.findings.push(finding(
                "datapack_identifier_duplicate",
                Severity::Warning,
                format!("{} · {id}", world),
                tr(
                    zh,
                    "Review repeated identifiers in this world's datapack list.",
                    "检查此世界数据包列表中的重复标识。",
                ),
                zh,
            ));
        }
        if known_format.is_none() {
            report.findings.push(finding(
                "datapack_format_unavailable",
                Severity::Info,
                format!(
                    "{} · {}",
                    world,
                    if id.is_empty() {
                        tr(zh, "identifier unavailable", "标识不可用")
                    } else {
                        id
                    }
                ),
                tr(
                    zh,
                    "Could not read this datapack’s format.",
                    "无法读取此数据包的格式。",
                ),
                zh,
            ));
        }
    }
    if items.is_empty() {
        report.findings.push(finding(
            "datapack_list_empty",
            Severity::Info,
            tr(
                zh,
                "No datapacks were listed for this game.",
                "此游戏没有列出数据包。",
            )
            .into(),
            tr(
                zh,
                "Install or select a world to review its datapacks.",
                "安装数据包或选择世界以查看列表。",
            ),
            zh,
        ));
    }
    if truncated {
        report.findings.push(finding(
            "datapack_list_truncated",
            Severity::Warning,
            tr(
                zh,
                "Some datapacks are not shown; totals are incomplete.",
                "部分数据包未显示，统计尚不完整。",
            )
            .into(),
            tr(
                zh,
                "Refresh to read the current list.",
                "刷新以重新读取列表。",
            ),
            zh,
        ));
    }
    report.findings.push(finding(
        "datapack_summary",
        Severity::Info,
        format!(
            "{} {} · {} {} · {} {}",
            items.len(),
            tr(zh, "listed", "项已列出"),
            missing_names,
            tr(zh, "missing names", "项缺少名称"),
            duplicates,
            tr(zh, "duplicate identifiers", "项标识重复")
        ),
        tr(
            zh,
            "Manage these entries in the world’s datapack page.",
            "可在对应世界的数据包页面管理这些条目。",
        ),
        zh,
    ));
    Ok(report)
}
struct DatapackAuditor;
impl exports::launcher::resource_report::reports::Guest for DatapackAuditor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        if cancelled() {
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
            if cancelled() {
                return Err(Failure::Cancelled);
            }
            let max = (metadata.byte_length as usize - bytes.len()).min(65536) as u32;
            let chunk = input.read_at(bytes.len() as u64, max)?;
            if chunk.is_empty() || chunk.len() > max as usize {
                return Err(Failure::Io);
            }
            bytes.extend_from_slice(&chunk);
        }
        let context = launcher::report_context::context::get();
        let report = analyze_bytes(&bytes, &context.locale)?;
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(DatapackAuditor);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_lists_worlds_and_reports_known_metadata_only() {
        let r = analyze_bytes(
            include_bytes!("../../../examples/datapack-auditor/fixtures/world-datapacks.json"),
            "en",
        )
        .unwrap();
        assert_eq!(r.tables[0].rows.len(), 3);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "datapack_identifier_duplicate")
        );
        assert!(r.findings.iter().any(|f| f.code == "datapack_name_missing"));
        assert_eq!(r.tables[0].rows[1][3], "Unavailable");
        assert!(r.findings.iter().any(|finding| {
            finding.code == "datapack_format_unavailable"
                && finding
                    .suggestions
                    .iter()
                    .any(|suggestion| suggestion.contains("Could not read this datapack’s format"))
        }));
    }
    #[test]
    fn truncation_is_explicit_and_chinese_copy_is_localized() {
        let raw =
            br#"{"schemaVersion":1,"scope":"datapacks","data":{"items":[],"truncated":true}}"#;
        let r = analyze_bytes(raw, "zh-CN").unwrap();
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "datapack_list_truncated")
        );
        assert_eq!(r.tables[0].columns[0], "世界");
    }
    #[test]
    fn rejects_wrong_scope_and_unbounded_input() {
        assert_eq!(
            analyze_bytes(
                br#"{"schemaVersion":1,"scope":"servers","data":{"items":[]}}"#,
                "en"
            )
            .unwrap_err(),
            Failure::Unsupported
        );
        assert_eq!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en").unwrap_err(),
            Failure::Limit
        );
    }
}

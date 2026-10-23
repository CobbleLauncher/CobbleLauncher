// SPDX-License-Identifier: MIT
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
use std::collections::BTreeSet;
const MAX_BYTES: usize = 2 * 1024 * 1024;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}
fn note(code: &str, severity: Severity, title: &str, message: String) -> Finding {
    Finding {
        code: code.into(),
        severity,
        title: title.into(),
        message,
        evidence: vec![],
        suggestions: vec![],
    }
}
fn snapshot(bytes: &[u8], scope: &str) -> Result<Value, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if value["schemaVersion"] != 1 || value["scope"] != scope || !value["data"]["items"].is_array()
    {
        return Err(Failure::Unsupported);
    }
    if value["data"]["items"].as_array().unwrap().len() > 2000 {
        return Err(Failure::Limit);
    }
    Ok(value)
}

fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    let root = snapshot(bytes, "packs")?;
    let zh = locale == "zh-CN";
    let data = &root["data"];
    let items = data["items"].as_array().unwrap();
    let mut findings = vec![];
    let mut rows = vec![];
    let available: BTreeSet<String> = items
        .iter()
        .filter(|v| v["kind"] == "resource_pack")
        .filter_map(|v| v["id"].as_str().map(str::to_owned))
        .collect();
    if let Some(selected) = data["selectedResourcePacks"]
        .as_array()
        .filter(|_| data["selectedResourcePacksAvailable"] != false)
    {
        if selected.len() > 2000 {
            return Err(Failure::Limit);
        }
        let mut seen = BTreeSet::new();
        for (index, entry) in selected.iter().enumerate() {
            let Some(id) = entry.as_str() else {
                return Err(Failure::Unsupported);
            };
            let short: String = id.chars().take(120).collect();
            let file = id.strip_prefix("file/");
            let exists = if id == "vanilla" {
                true
            } else {
                file.is_some_and(|f| available.contains(f))
            };
            let status = if !seen.insert(id) {
                findings.push(note(
                    "stack_duplicate",
                    Severity::Warning,
                    tr(zh, "Repeated pack", "重复资源包"),
                    short.clone(),
                ));
                tr(zh, "Repeated", "重复")
            } else if file.is_some() && !exists && data["truncated"] != true {
                findings.push(note(
                    "stack_missing",
                    Severity::Warning,
                    tr(zh, "Selected pack missing", "所选资源包缺失"),
                    short.clone(),
                ));
                tr(zh, "Missing", "缺失")
            } else if exists {
                tr(zh, "Available", "可用")
            } else if file.is_some() {
                tr(zh, "Not collected", "未收集")
            } else {
                tr(zh, "External or built-in", "外部或内置")
            };
            rows.push(vec![(index + 1).to_string(), short, status.into()]);
        }
    } else {
        findings.push(note(
            "stack_unavailable",
            Severity::Info,
            tr(zh, "Selection unavailable", "无法读取所选顺序"),
            tr(
                zh,
                "The resourcePacks selection was absent or could not be read.",
                "resourcePacks 选择缺失或无法读取。",
            )
            .into(),
        ));
    }
    let inventory = items
        .iter()
        .take(500)
        .map(|item| {
            vec![
                text(item, "name"),
                text(item, "id"),
                text(item, "kind"),
                item["packFormat"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| tr(zh, "Unknown", "未知").into()),
            ]
        })
        .collect();
    if data["truncated"] == true {
        findings.push(note(
            "inventory_truncated",
            Severity::Warning,
            tr(zh, "Partial inventory", "清单不完整"),
            tr(
                zh,
                "Some pack entries were not collected; missing-pack results may be incomplete.",
                "部分资源包未被收集，缺失检查可能不完整。",
            )
            .into(),
        ));
    }
    if items.len() > 500 || rows.len() > 500 {
        findings.push(note(
            "report_preview",
            Severity::Info,
            tr(zh, "Report preview", "报告预览"),
            tr(
                zh,
                "Tables show at most 500 entries each. Checks use the collected inventory.",
                "每张表最多显示 500 条，检查使用已收集的清单。",
            )
            .into(),
        ));
    }
    findings.truncate(60);
    rows.truncate(500);
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![
            Table {
                columns: vec![
                    tr(zh, "Order", "顺序").into(),
                    tr(zh, "Selected pack", "所选资源包").into(),
                    tr(zh, "State", "状态").into(),
                ],
                rows,
            },
            Table {
                columns: vec![
                    tr(zh, "Pack", "资源包").into(),
                    tr(zh, "File", "文件").into(),
                    tr(zh, "Kind", "类型").into(),
                    tr(zh, "Declared format", "声明格式").into(),
                ],
                rows: inventory,
            },
        ],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_output_is_bounded_for_large_unicode_inventory() {
        let items=(0..600).map(|n|serde_json::json!({"id":format!("{n}{}","界".repeat(300)),"name":"中".repeat(500),"kind":"resource_pack"})).collect::<Vec<_>>();
        let raw = serde_json::to_vec(
            &serde_json::json!({"schemaVersion":1,"scope":"packs","data":{"items":items}}),
        )
        .unwrap();
        let report = analyze_bytes(&raw, "zh-CN").unwrap();
        assert!(report.findings.len() <= 64);
        assert!(report.tables.iter().all(|t| t.rows.len() <= 500));
        assert!(
            report
                .tables
                .iter()
                .flat_map(|t| &t.rows)
                .flatten()
                .all(|cell| cell.len() <= 2048)
        );
    }
    #[test]
    fn ordered_missing_duplicate_and_builtin() {
        let r=analyze_bytes(br#"{"schemaVersion":1,"scope":"packs","data":{"items":[{"id":"a.zip","kind":"resource_pack","packFormat":18}],"selectedResourcePacks":["vanilla","file/a.zip","file/gone.zip","file/a.zip","fabric"]}}"#,"en").unwrap();
        assert!(r.findings.iter().any(|f| f.code == "stack_missing"));
        assert!(r.findings.iter().any(|f| f.code == "stack_duplicate"));
        assert_eq!(r.tables[0].rows[4][2], "External or built-in");
        assert_eq!(r.tables[1].rows[0][3], "18");
    }
    #[test]
    fn absent_selection_not_empty_success() {
        let r = analyze_bytes(
            br#"{"schemaVersion":1,"scope":"packs","data":{"items":[],"selectedResourcePacks":[],"selectedResourcePacksAvailable":false}}"#,
            "zh-CN",
        )
        .unwrap();
        assert_eq!(r.findings[0].code, "stack_unavailable");
    }
    #[test]
    fn wrong_scope_and_limit() {
        assert!(matches!(
            analyze_bytes(b"{}", "en"),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
    }
}

struct Analyzer;
impl exports::launcher::resource_report::reports::Guest for Analyzer {
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
            let max = (metadata.byte_length as usize - bytes.len()).min(65536) as u32;
            let chunk = input.read_at(bytes.len() as u64, max)?;
            if chunk.is_empty() || chunk.len() > max as usize {
                return Err(Failure::Io);
            }
            bytes.extend_from_slice(&chunk);
        }
        let ctx = launcher::report_context::context::get();
        let report = analyze_bytes(&bytes, &ctx.locale)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(Analyzer);

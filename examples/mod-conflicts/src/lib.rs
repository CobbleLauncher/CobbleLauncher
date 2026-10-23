// SPDX-License-Identifier: MIT
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
use std::collections::BTreeMap;

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
    let root = snapshot(bytes, "mods")?;
    let zh = locale == "zh-CN";
    let items = root["data"]["items"].as_array().unwrap();
    let mut findings = vec![];
    let mut rows = vec![];
    for item in items {
        rows.push(vec![
            text(item, "name"),
            text(item, "id"),
            text(item, "version"),
            text(item, "loader"),
            tr(
                zh,
                if item["disabled"] == true {
                    "Disabled"
                } else {
                    "Enabled"
                },
                if item["disabled"] == true {
                    "已禁用"
                } else {
                    "已启用"
                },
            )
            .into(),
        ]);
    }
    let mut filenames: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for item in items {
        let id = item["id"].as_str().unwrap_or("");
        if id.is_empty() {
            continue;
        }
        let base = if item["disabled"] == true {
            id.strip_suffix(".disabled").unwrap_or(id)
        } else {
            id
        };
        filenames.entry(base.to_lowercase()).or_default().push(item);
    }
    for group in filenames.values().filter(|g| g.len() > 1) {
        let enabled = group.iter().filter(|v| v["disabled"] != true).count();
        let names = group
            .iter()
            .map(|v| text(v, "id"))
            .collect::<Vec<_>>()
            .join(", ")
            .chars()
            .take(500)
            .collect::<String>();
        findings.push(note(
            if enabled > 1 {
                "filename_collision"
            } else {
                "disabled_copy"
            },
            if enabled > 1 {
                Severity::Warning
            } else {
                Severity::Info
            },
            tr(
                zh,
                if enabled > 1 {
                    "File name collision"
                } else {
                    "Copies with the same file name"
                },
                if enabled > 1 {
                    "文件名冲突"
                } else {
                    "同名文件副本"
                },
            ),
            names,
        ));
    }
    let active = items
        .iter()
        .filter(|item| item["disabled"] != true)
        .collect::<Vec<_>>();
    let mut declared: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for item in &active {
        if let Some(ids) = item["declaredIds"].as_array() {
            for id in ids.iter().filter_map(Value::as_str) {
                let owners = declared.entry(id.into()).or_default();
                if !owners.iter().any(|owner| std::ptr::eq(*owner, *item)) {
                    owners.push(item);
                }
            }
        }
    }
    for (id, owners) in declared.iter().filter(|(_, owners)| owners.len() > 1) {
        findings.push(note(
            "duplicate_declared_id",
            Severity::Warning,
            tr(zh, "Duplicate mod ID", "重复模组 ID"),
            format!(
                "{}: {}",
                id.chars().take(120).collect::<String>(),
                owners
                    .iter()
                    .map(|item| text(item, "id"))
                    .collect::<Vec<_>>()
                    .join(", ")
                    .chars()
                    .take(380)
                    .collect::<String>()
            ),
        ));
    }
    let complete = root["data"]["truncated"] != true
        && active.iter().all(|item| {
            item["declaredIds"]
                .as_array()
                .is_some_and(|ids| !ids.is_empty())
        });
    if complete {
        for item in &active {
            if let Some(dependencies) = item["dependencies"].as_array() {
                for dependency in dependencies.iter().filter(|dep| dep["optional"] != true) {
                    let Some(id) = dependency["id"].as_str() else {
                        continue;
                    };
                    if ![
                        "minecraft",
                        "java",
                        "fabricloader",
                        "quilt_loader",
                        "forge",
                        "neoforge",
                    ]
                    .contains(&id)
                        && !declared.contains_key(id)
                    {
                        findings.push(note(
                            "dependency_missing",
                            Severity::Warning,
                            tr(zh, "Required mod missing", "缺少必需模组"),
                            format!(
                                "{} → {}",
                                text(item, "name"),
                                id.chars().take(120).collect::<String>()
                            ),
                        ));
                    }
                }
            }
        }
    }
    if root["data"]["truncated"] == true {
        findings.push(note(
            "inventory_truncated",
            Severity::Warning,
            tr(zh, "Partial inventory", "清单不完整"),
            tr(
                zh,
                "Only the collected entries are shown.",
                "仅显示已收集的条目。",
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
        tables: vec![Table {
            columns: vec![
                tr(zh, "Mod", "模组").into(),
                tr(zh, "File", "文件").into(),
                tr(zh, "Version", "版本").into(),
                tr(zh, "Loader", "加载器").into(),
                tr(zh, "State", "状态").into(),
            ],
            rows,
        }],
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_output_is_bounded_for_large_unicode_inventory() {
        let items=(0..600).map(|n|serde_json::json!({"id":format!("{n}{}","界".repeat(300)),"name":"中".repeat(500),"kind":"resource_pack"})).collect::<Vec<_>>();
        let raw = serde_json::to_vec(
            &serde_json::json!({"schemaVersion":1,"scope":"mods","data":{"items":items}}),
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
    fn no_conflicts_inferred_from_filenames_or_loader_inventory() {
        let r=analyze_bytes(br#"{"schemaVersion":1,"scope":"mods","data":{"items":[{"id":"same-1.jar","name":"A","loader":"fabric"},{"id":"same-2.jar","name":"A","loader":"forge","disabled":true}]}}"#,"en").unwrap();
        assert!(r.findings.is_empty());
        assert_eq!(r.tables[0].rows.len(), 2);
        assert_eq!(r.tables[0].rows[1][4], "Disabled");
    }
    #[test]
    fn active_case_collisions_and_disabled_copies_are_distinct() {
        let r=analyze_bytes(br#"{"schemaVersion":1,"scope":"mods","data":{"items":[{"id":"A.jar"},{"id":"a.jar"},{"id":"B.jar"},{"id":"B.jar.disabled","disabled":true}]}}"#,"en").unwrap();
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "filename_collision" && matches!(f.severity, Severity::Warning))
        );
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "disabled_copy" && matches!(f.severity, Severity::Info))
        );
    }
    #[test]
    fn declared_ids_and_dependency_presence_use_known_metadata_only() {
        let bytes=br#"{"schemaVersion":1,"scope":"mods","data":{"items":[{"id":"a.jar","declaredIds":["a"],"dependencies":[{"id":"java"},{"id":"missing"}]},{"id":"b.jar","declaredIds":["a"]},{"id":"c.jar","disabled":true,"declaredIds":["c"]}]}}"#;
        let r = analyze_bytes(bytes, "en").unwrap();
        assert!(r.findings.iter().any(|f| f.code == "duplicate_declared_id"));
        assert_eq!(
            r.findings
                .iter()
                .filter(|f| f.code == "dependency_missing")
                .count(),
            1
        );
        let bytes=br#"{"schemaVersion":1,"scope":"mods","data":{"items":[{"id":"a.jar","declaredIds":["a"],"dependencies":[{"id":"unknown"}]},{"id":"unknown.jar","declaredIds":[]}]}}"#;
        assert!(
            !analyze_bytes(bytes, "en")
                .unwrap()
                .findings
                .iter()
                .any(|f| f.code == "dependency_missing")
        );
    }
    #[test]
    fn bounds_and_scope() {
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
        assert!(matches!(
            analyze_bytes(
                br#"{"schemaVersion":1,"scope":"packs","data":{"items":[]}}"#,
                "en"
            ),
            Err(Failure::Unsupported)
        ));
    }
    #[test]
    fn localized_partial_inventory() {
        let r = analyze_bytes(
            br#"{"schemaVersion":1,"scope":"mods","data":{"items":[],"truncated":true}}"#,
            "zh-CN",
        )
        .unwrap();
        assert!(
            r.findings
                .iter()
                .any(|finding| finding.code == "inventory_truncated"
                    && finding.title == "清单不完整")
        );
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

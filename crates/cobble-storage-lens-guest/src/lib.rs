// SPDX-License-Identifier: MIT
//! Read-only storage summary for the bounded, native-created game snapshot.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});

use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;

const MAX_SNAPSHOT_BYTES: usize = 256 * 1024;
const MAX_ITEMS: usize = 64;
const MAX_ROWS: usize = 12;

#[derive(Clone, Debug)]
struct Category {
    id: String,
    size: u64,
    files: u64,
}

fn locale_is_zh(locale: &str) -> bool {
    locale == "zh-CN"
}
fn text<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn number(value: &Value) -> Option<u64> {
    value.as_u64()
}
fn parse_snapshot(bytes: &[u8]) -> Result<(Vec<Category>, bool, Option<String>), Failure> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(Failure::Limit);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if root["schemaVersion"] != 1 || root["scope"] != "storage" {
        return Err(Failure::Unsupported);
    }
    if !root["game"].is_object() || !root["revision"].is_string() {
        return Err(Failure::Unsupported);
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or(Failure::Unsupported)?;
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .ok_or(Failure::Unsupported)?;
    if items.len() > MAX_ITEMS {
        return Err(Failure::Limit);
    }
    let mut categories = Vec::with_capacity(items.len());
    for item in items {
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .filter(|v| {
                matches!(
                    *v,
                    "mods"
                        | "resourcepacks"
                        | "shaderpacks"
                        | "saves"
                        | "screenshots"
                        | "logs"
                        | "config"
                )
            })
            .ok_or(Failure::Unsupported)?;
        if item
            .get("name")
            .and_then(Value::as_str)
            .is_none_or(|name| name.len() > 100)
        {
            return Err(Failure::Unsupported);
        }
        let size = number(&item["sizeBytes"]).ok_or(Failure::Unsupported)?;
        let files = number(&item["fileCount"]).ok_or(Failure::Unsupported)?;
        if categories
            .iter()
            .any(|category: &Category| category.id == id)
        {
            return Err(Failure::Unsupported);
        }
        categories.push(Category {
            id: id.to_owned(),
            size,
            files,
        });
    }
    categories.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
    let truncated = data
        .get("truncated")
        .and_then(Value::as_bool)
        .ok_or(Failure::Unsupported)?;
    let unavailable = data
        .get("unavailable")
        .and_then(Value::as_str)
        .map(|v| v.chars().take(200).collect());
    Ok((categories, truncated, unavailable))
}
fn category_name(id: &str, zh: bool) -> String {
    match (id, zh) {
        ("mods", true) => "模组".into(),
        ("resourcepacks", true) => "资源包".into(),
        ("shaderpacks", true) => "光影包".into(),
        ("saves", true) => "世界存档".into(),
        ("screenshots", true) => "截图".into(),
        ("logs", true) => "日志".into(),
        ("config", true) => "配置".into(),
        ("mods", false) => "Mods".into(),
        ("resourcepacks", false) => "Resource packs".into(),
        ("shaderpacks", false) => "Shader packs".into(),
        ("saves", false) => "World saves".into(),
        ("screenshots", false) => "Screenshots".into(),
        ("logs", false) => "Logs".into(),
        ("config", false) => "Config".into(),
        (_, _) => id.chars().take(64).collect(),
    }
}
fn format_bytes(value: u64, zh: bool) -> String {
    let (unit, divisor) = if value >= 1024_u64.pow(3) {
        ("GiB", 1024_f64.powi(3))
    } else if value >= 1024_u64.pow(2) {
        ("MiB", 1024_f64.powi(2))
    } else if value >= 1024 {
        ("KiB", 1024_f64)
    } else {
        return format!("{value} {}", text(zh, "B", "字节"));
    };
    format!("{:.1} {unit}", value as f64 / divisor)
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    let (categories, truncated, unavailable) = parse_snapshot(bytes)?;
    let zh = locale_is_zh(locale);
    let mut total_bytes = 0_u64;
    let mut total_files = 0_u64;
    for category in &categories {
        total_bytes = total_bytes
            .checked_add(category.size)
            .ok_or(Failure::Limit)?;
        total_files = total_files
            .checked_add(category.files)
            .ok_or(Failure::Limit)?;
    }
    let rows = categories
        .iter()
        .take(MAX_ROWS)
        .map(|category| {
            vec![
                category_name(&category.id, zh),
                category.files.to_string(),
                format_bytes(category.size, zh),
            ]
        })
        .collect();
    let largest = categories
        .iter()
        .take(3)
        .filter(|category| category.size > 0)
        .map(|category| {
            format!(
                "{} ({})",
                category_name(&category.id, zh),
                format_bytes(category.size, zh)
            )
        })
        .collect::<Vec<_>>()
        .join(if zh { "、" } else { ", " });
    let summary = if largest.is_empty() {
        format!(
            "{} · {} {}",
            format_bytes(total_bytes, zh),
            total_files,
            text(zh, "files across managed categories", "个受管目录中的文件")
        )
    } else {
        format!(
            "{} · {} {} · {}: {}",
            format_bytes(total_bytes, zh),
            total_files,
            text(zh, "files across managed categories", "个受管目录中的文件"),
            text(zh, "Largest", "占用较大"),
            largest
        )
    };
    let mut findings = vec![Finding {
        code: "storage_summary".into(),
        severity: Severity::Info,
        title: text(zh, "Game storage", "游戏存储").into(),
        message: summary,
        evidence: vec![],
        suggestions: vec![],
    }];
    if truncated {
        findings.push(Finding {
            code: "storage_partial".into(),
            severity: Severity::Warning,
            title: text(zh, "Partial inventory", "目录统计不完整").into(),
            message: text(
                zh,
                "Some entries were not counted; shown totals are partial.",
                "部分条目未计入，当前总数仅供参考。未完整统计。",
            )
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if categories.first().is_some_and(|category| category.size > 0) {
        findings.push(Finding {
            code: "storage_review".into(),
            severity: Severity::Info,
            title: text(zh, "Review large categories", "检查占用较大的目录").into(),
            message: text(
                zh,
                "Review old screenshots or worlds in the launcher before cleanup.",
                "清理前可在启动器中检查旧截图或世界存档。",
            )
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if unavailable.is_some() {
        findings.push(Finding {
            code: "storage_unavailable".into(),
            severity: Severity::Warning,
            title: text(zh, "Some categories unavailable", "部分目录无法统计").into(),
            message: text(
                zh,
                "Some folders could not be counted.",
                "部分文件夹无法统计。",
            )
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![Table {
            columns: vec![
                text(zh, "Category", "类别").into(),
                text(zh, "Files", "文件数").into(),
                text(zh, "Size", "大小").into(),
            ],
            rows,
        }],
    })
}
fn read_input(input: &Input) -> Result<(String, Vec<u8>), Failure> {
    let metadata = input.describe()?;
    if metadata.media_type != "application/json" {
        return Err(Failure::Unsupported);
    }
    if metadata.byte_length > MAX_SNAPSHOT_BYTES as u64 {
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
    Ok((metadata.media_type, bytes))
}
struct StorageLens;
impl exports::launcher::resource_report::reports::Guest for StorageLens {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let (_, bytes) = read_input(input)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let context = launcher::report_context::context::get();
        analyze_bytes(&bytes, &context.locale)
    }
}
export!(StorageLens);

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(data: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"scope":"storage","game":{"id":"g1","name":"Test","version":"1.21.1"},"revision":"a".repeat(64),"data":data})).unwrap()
    }
    #[test]
    fn sorts_categories_reports_partial_inventory_and_never_advises_deletion() {
        let input = snapshot(
            serde_json::json!({"items":[{"id":"logs","name":"Logs","sizeBytes":2,"fileCount":1},{"id":"saves","name":"Saves","sizeBytes":2048,"fileCount":3}],"truncated":true}),
        );
        let report = analyze_bytes(&input, "en").unwrap();
        assert_eq!(report.tables[0].rows[0][0], "World saves");
        assert!(report.findings[0].message.contains("World saves"));
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "storage_partial")
        );
        assert!(report.findings.iter().all(|finding| {
            !finding
                .suggestions
                .iter()
                .any(|s| s.to_lowercase().contains("delete"))
        }));
    }
    #[test]
    fn rejects_wrong_scope_duplicate_categories_and_oversized_item_lists() {
        let mut input: Value =
            serde_json::from_slice(&snapshot(serde_json::json!({"items":[],"truncated":false})))
                .unwrap();
        input["scope"] = Value::String("runtime".into());
        assert!(matches!(
            parse_snapshot(&serde_json::to_vec(&input).unwrap()),
            Err(Failure::Unsupported)
        ));
        let duplicate = snapshot(
            serde_json::json!({"items":[{"id":"mods","name":"Mods","sizeBytes":1,"fileCount":1},{"id":"mods","name":"Mods","sizeBytes":2,"fileCount":2}],"truncated":false}),
        );
        assert!(matches!(
            parse_snapshot(&duplicate),
            Err(Failure::Unsupported)
        ));
        let oversized = snapshot(
            serde_json::json!({"items":vec![Value::Null; MAX_ITEMS + 1],"truncated":false}),
        );
        assert!(matches!(parse_snapshot(&oversized), Err(Failure::Limit)));
    }
}

// SPDX-License-Identifier: MIT
//! Read-only serverbook name and host-classification report. Address values are never emitted.
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
        title: tr(zh, "Server list", "服务器列表").into(),
        message,
        evidence: vec![],
        suggestions: vec![advice.into()],
    }
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let zh = locale == "zh-CN";
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1)
        || value.get("scope").and_then(Value::as_str) != Some("servers")
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
                tr(zh, "Entry", "条目").into(),
                tr(zh, "Server name", "服务器名称").into(),
                tr(zh, "Address category", "地址类别").into(),
                tr(zh, "Duplicate", "重复项").into(),
            ],
            rows: vec![],
        }],
    };
    let mut missing = 0usize;
    let mut duplicate = 0usize;
    for (index, item) in items.iter().take(4096).enumerate() {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let ordinal = item
            .get("id")
            .and_then(Value::as_u64)
            .unwrap_or(index as u64);
        let category = item
            .get("addressKind")
            .and_then(Value::as_str)
            .unwrap_or("");
        let category = match category {
            "local" => tr(zh, "Local", "本地"),
            "public" => tr(zh, "Public", "公网"),
            "unknown" => tr(zh, "Unknown", "未知"),
            _ => tr(zh, "Unknown", "未知"),
        };
        let duplicate_of = item
            .get("duplicateOf")
            .and_then(Value::as_u64)
            .filter(|prior| *prior < ordinal);
        report.tables[0].rows.push(vec![
            (ordinal + 1).to_string(),
            if name.is_empty() {
                tr(zh, "Unnamed server", "未命名服务器").into()
            } else {
                name.into()
            },
            category.into(),
            duplicate_of
                .map(|prior| format!("{} {}", tr(zh, "matches entry", "与条目重复"), prior + 1))
                .unwrap_or_else(|| tr(zh, "No", "否").into()),
        ]);
        if name.is_empty() {
            missing += 1;
            report.findings.push(finding(
                "server_name_missing",
                Severity::Warning,
                format!("{} {}", tr(zh, "Entry", "条目"), ordinal + 1),
                tr(
                    zh,
                    "Add a name to make this saved server easier to recognize.",
                    "为已保存的服务器添加名称，便于识别。",
                ),
                zh,
            ));
        }
        if duplicate_of.is_some() {
            duplicate += 1;
            report.findings.push(finding(
                "server_duplicate",
                Severity::Warning,
                format!("{} {}", tr(zh, "Entry", "条目"), ordinal + 1),
                tr(
                    zh,
                    "Review the duplicate group reported by the host.",
                    "检查宿主报告的重复组。",
                ),
                zh,
            ));
        }
    }
    if items.is_empty() {
        report.findings.push(finding(
            "server_list_empty",
            Severity::Info,
            tr(zh, "No saved servers were listed.", "没有已保存的服务器。").into(),
            tr(
                zh,
                "Add a server in the launcher or game to review the list.",
                "在启动器或游戏中添加服务器后即可查看列表。",
            ),
            zh,
        ));
    }
    if truncated {
        report.findings.push(finding(
            "server_list_truncated",
            Severity::Warning,
            tr(
                zh,
                "The host limited this server list; totals may be incomplete.",
                "宿主限制了服务器列表，统计可能不完整。",
            )
            .into(),
            tr(
                zh,
                "Refresh the list to inspect the current host snapshot.",
                "刷新列表以检查当前宿主快照。",
            ),
            zh,
        ));
    }
    report.findings.push(finding("server_summary",Severity::Info,format!("{} {} · {} {} · {} {}",items.len(),tr(zh,"listed","项已列出"),missing,tr(zh,"unnamed","项未命名"),duplicate,tr(zh,"duplicates","项为重复项")),tr(zh,"Only names and host-provided address categories are shown. Addresses are not included; this tool does not connect to servers.","仅显示名称和宿主提供的地址类别，不包含地址；此工具不会连接服务器。"),zh));
    Ok(report)
}
struct ServerbookInspector;
impl exports::launcher::resource_report::reports::Guest for ServerbookInspector {
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
export!(ServerbookInspector);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_uses_host_duplicate_group_and_never_returns_address_fields() {
        let bytes = include_bytes!("../../../examples/serverbook-inspector/fixtures/servers.json");
        let r = analyze_bytes(bytes, "en").unwrap();
        assert_eq!(r.tables[0].rows.len(), 3);
        assert!(r.findings.iter().any(|f| f.code == "server_duplicate"));
        assert!(r.findings.iter().any(|f| f.code == "server_name_missing"));
        let output = r
            .tables
            .iter()
            .flat_map(|table| {
                table
                    .columns
                    .iter()
                    .chain(table.rows.iter().flatten())
                    .cloned()
            })
            .chain(r.findings.iter().flat_map(|finding| {
                std::iter::once(finding.code.clone())
                    .chain(std::iter::once(finding.title.clone()))
                    .chain(std::iter::once(finding.message.clone()))
                    .chain(finding.suggestions.iter().cloned())
            }))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!output.contains("sensitive.example.invalid"));
    }
    #[test]
    fn truncation_and_localized_copy_are_reported() {
        let raw = br#"{"schemaVersion":1,"scope":"servers","data":{"items":[],"truncated":true}}"#;
        let r = analyze_bytes(raw, "zh-CN").unwrap();
        assert!(r.findings.iter().any(|f| f.code == "server_list_truncated"));
        assert_eq!(r.tables[0].columns[1], "服务器名称");
    }
    #[test]
    fn rejects_wrong_scope_and_oversize_input() {
        assert!(matches!(
            analyze_bytes(
                br#"{"schemaVersion":1,"scope":"datapacks","data":{"items":[]}}"#,
                "en"
            ),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
    }
}

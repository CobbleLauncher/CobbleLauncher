// SPDX-License-Identifier: MIT
//! Read-only, evidence-based report over the bounded runtime settings snapshot.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});

use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
use std::collections::BTreeMap;

const MAX_SNAPSHOT_BYTES: usize = 128 * 1024;
const MAX_ITEMS: usize = 32;
const MIN_MEMORY_MB: u64 = 512;
const MAX_MEMORY_MB: u64 = 262_144;

fn zh(locale: &str) -> bool {
    locale == "zh-CN"
}
fn text<'a>(is_zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if is_zh { cn } else { en }
}
fn parse_snapshot(bytes: &[u8]) -> Result<BTreeMap<String, Value>, Failure> {
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(Failure::Limit);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if root["schemaVersion"] != 1
        || root["scope"] != "runtime"
        || !root["game"].is_object()
        || !root["revision"].is_string()
    {
        return Err(Failure::Unsupported);
    }
    let data = root
        .get("data")
        .and_then(Value::as_object)
        .ok_or(Failure::Unsupported)?;
    if data.get("truncated").and_then(Value::as_bool) != Some(false) {
        return Err(Failure::Unsupported);
    }
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .ok_or(Failure::Unsupported)?;
    if items.len() > MAX_ITEMS {
        return Err(Failure::Limit);
    }
    let mut values = BTreeMap::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .filter(|v| v.len() <= 64)
            .ok_or(Failure::Unsupported)?;
        let value = item.get("value").ok_or(Failure::Unsupported)?;
        if values.insert(name.to_owned(), value.clone()).is_some() {
            return Err(Failure::Unsupported);
        }
    }
    Ok(values)
}
fn display(value: &Value, is_zh: bool) -> String {
    match value {
        Value::Null => text(is_zh, "Inherited or unavailable", "继承设置或暂不可用").into(),
        Value::Bool(value) => text(
            is_zh,
            if *value { "Yes" } else { "No" },
            if *value { "是" } else { "否" },
        )
        .into(),
        Value::String(value) => value
            .chars()
            .filter(|c| !c.is_control())
            .take(160)
            .collect(),
        Value::Number(value) => value.to_string(),
        _ => text(is_zh, "Not shown", "不显示").into(),
    }
}
fn number(values: &BTreeMap<String, Value>, key: &str) -> Option<u64> {
    values.get(key)?.as_u64()
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    let values = parse_snapshot(bytes)?;
    let is_zh = zh(locale);
    let required_java = number(&values, "requiredJavaMajor");
    let current_java = number(&values, "selectedJavaMajor");
    let mut findings = Vec::new();
    if let (Some(required), Some(current)) = (required_java, current_java) {
        if required != current {
            findings.push(Finding {
                code: "java_major_mismatch".into(),
                severity: Severity::Warning,
                title: text(is_zh, "Java version mismatch", "Java 版本不匹配").into(),
                message: format!(
                    "{} {current}; {} {required}.",
                    text(is_zh, "Selected Java major:", "所选 Java 主版本："),
                    text(
                        is_zh,
                        "required by installed game metadata:",
                        "游戏元数据要求："
                    )
                ),
                evidence: vec![],
                suggestions: vec![
                    text(
                        is_zh,
                        "Review this game's Java selection in Runtime settings.",
                        "请在运行环境设置中检查此游戏选择的 Java。",
                    )
                    .into(),
                ],
            });
        } else {
            findings.push(Finding {
                code: "java_major_match".into(),
                severity: Severity::Info,
                title: text(is_zh, "Java requirement", "Java 要求").into(),
                message: format!(
                    "{} {current}.",
                    text(
                        is_zh,
                        "Selected Java major matches the installed metadata requirement:",
                        "所选 Java 主版本符合已安装元数据要求："
                    )
                ),
                evidence: vec![],
                suggestions: vec![],
            });
        }
    } else {
        findings.push(Finding {
            code: "java_major_unknown".into(),
            severity: Severity::Info,
            title: text(is_zh, "Java version not verified", "Java 版本尚未验证").into(),
            message: text(
                is_zh,
                "Java version information is incomplete.",
                "Java 版本信息尚不完整。",
            )
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if let Some(memory) = number(&values, "memoryMb") {
        if !(MIN_MEMORY_MB..=MAX_MEMORY_MB).contains(&memory) {
            findings.push(Finding {
                code: "memory_out_of_bounds".into(),
                severity: Severity::Warning,
                title: text(
                    is_zh,
                    "Memory setting outside launcher bounds",
                    "内存设置超出启动器范围",
                )
                .into(),
                message: format!("{memory} MiB; {}–{} MiB.", MIN_MEMORY_MB, MAX_MEMORY_MB),
                evidence: vec![],
                suggestions: vec![
                    text(
                        is_zh,
                        "Choose a value within the range shown in Runtime settings.",
                        "请在运行环境设置中选择范围内的数值。",
                    )
                    .into(),
                ],
            });
        }
    }
    if values.get("javaMode").and_then(Value::as_str) == Some("custom") && current_java.is_none() {
        findings.push(Finding {
            code: "custom_java_unverified".into(),
            severity: Severity::Info,
            title: text(is_zh, "Custom Java not probed", "自定义 Java 未探测").into(),
            message: text(
                is_zh,
                "Rescan Java in Runtime settings to check the selected version.",
                "请在运行环境设置中重新扫描 Java，检查所选版本。",
            )
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    let mut rows = Vec::new();
    for key in [
        "minecraftVersion",
        "loader",
        "loaderVersion",
        "memoryMb",
        "javaMode",
        "selectedJavaMajor",
        "requiredJavaMajor",
        "launchState",
        "jvmArgumentCount",
    ] {
        if let Some(value) = values.get(key) {
            let label = match (key, is_zh) {
                ("minecraftVersion", true) => "Minecraft 版本",
                ("loader", true) => "加载器",
                ("loaderVersion", true) => "加载器版本",
                ("memoryMb", true) => "内存（MiB）",
                ("javaMode", true) => "Java 选择模式",
                ("selectedJavaMajor", true) => "所选 Java 主版本",
                ("requiredJavaMajor", true) => "元数据要求的 Java 主版本",
                ("launchState", true) => "启动状态",
                ("jvmArgumentCount", true) => "JVM 参数数量",
                ("minecraftVersion", false) => "Minecraft version",
                ("loader", false) => "Loader",
                ("loaderVersion", false) => "Loader version",
                ("memoryMb", false) => "Memory (MiB)",
                ("javaMode", false) => "Java selection mode",
                ("selectedJavaMajor", false) => "Selected Java major",
                ("requiredJavaMajor", false) => "Java major required by metadata",
                ("launchState", false) => "Launch state",
                _ => "JVM argument count",
            };
            rows.push(vec![label.into(), display(value, is_zh)]);
        }
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![Table {
            columns: vec![
                text(is_zh, "Setting", "设置").into(),
                text(is_zh, "Value", "当前值").into(),
            ],
            rows,
        }],
    })
}
fn read_input(input: &Input) -> Result<Vec<u8>, Failure> {
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
    Ok(bytes)
}
struct RuntimeAdvisor;
impl exports::launcher::resource_report::reports::Guest for RuntimeAdvisor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let bytes = read_input(input)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        analyze_bytes(&bytes, &launcher::report_context::context::get().locale)
    }
}
export!(RuntimeAdvisor);

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(items: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"scope":"runtime","game":{"id":"g1","name":"Test","version":"1.21.1"},"revision":"b".repeat(64),"data":{"items":items,"truncated":false}})).unwrap()
    }
    #[test]
    fn reports_only_known_java_mismatch_and_memory_bounds() {
        let bytes = snapshot(serde_json::json!([
            {"name":"requiredJavaMajor","value":21},{"name":"selectedJavaMajor","value":17},
            {"name":"memoryMb","value":128},{"name":"javaMode","value":"custom"},{"name":"jvmArgumentCount","value":3}
        ]));
        let report = analyze_bytes(&bytes, "en").unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "java_major_mismatch")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "memory_out_of_bounds")
        );
        assert!(
            !report.tables[0]
                .rows
                .iter()
                .any(|row| row.iter().any(|cell| cell.contains("path")))
        );
    }
    #[test]
    fn does_not_infer_a_java_mismatch_when_snapshot_lacks_java_evidence() {
        let bytes = snapshot(
            serde_json::json!([{"name":"javaMode","value":"automatic"},{"name":"memoryMb","value":null}]),
        );
        let report = analyze_bytes(&bytes, "zh-CN").unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "java_major_unknown")
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.code == "java_major_mismatch")
        );
    }
    #[test]
    fn rejects_wrong_scope_and_duplicate_runtime_fields() {
        let mut root: Value = serde_json::from_slice(&snapshot(serde_json::json!([]))).unwrap();
        root["scope"] = Value::String("storage".into());
        assert!(matches!(
            parse_snapshot(&serde_json::to_vec(&root).unwrap()),
            Err(Failure::Unsupported)
        ));
        let duplicate = snapshot(
            serde_json::json!([{"name":"javaMode","value":"inherit"},{"name":"javaMode","value":"custom"}]),
        );
        assert!(matches!(
            parse_snapshot(&duplicate),
            Err(Failure::Unsupported)
        ));
    }
}

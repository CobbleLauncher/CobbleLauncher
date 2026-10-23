// SPDX-License-Identifier: MIT
//! Bounded host snapshot only: relative names and immutable selected file text.
use super::exports::launcher::resource_report::reports::Table;
use super::{Failure, Finding, Report, Severity, analyze_bytes_with_options, info, tr};
use serde_json::Value;
use std::collections::{BTreeSet, VecDeque};
pub const MEDIA_TYPE: &str = "application/vnd.cobble.config-folder+json";
pub const MAX_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;
const MAX_DETAILS: usize = 60;
fn relative_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 240
        && !name.contains(['\\', ':'])
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| !matches!(part, "" | "." | ".."))
}
#[cfg(test)]
pub fn analyze(
    bytes: &[u8],
    format: &str,
    locale: &str,
    cancelled: impl Fn() -> bool,
) -> Result<Report, Failure> {
    analyze_with_options(bytes, format, locale, cancelled, true)
}
pub fn analyze_with_options(
    bytes: &[u8],
    format: &str,
    locale: &str,
    cancelled: impl Fn() -> bool,
    warn_duplicates: bool,
) -> Result<Report, Failure> {
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return Err(Failure::Limit);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if value["schema_version"].as_u64() != Some(1) {
        return Err(Failure::Unsupported);
    }
    let files = value["files"].as_array().ok_or(Failure::Unsupported)?;
    let skipped = value["skipped"]
        .as_u64()
        .filter(|n| *n <= u32::MAX as u64)
        .ok_or(Failure::Unsupported)?;
    let truncated = value["truncated"].as_bool().ok_or(Failure::Unsupported)?;
    if files.len() > 64 {
        return Err(Failure::Limit);
    }
    let zh = locale == "zh-CN";
    let mut file_table = Table {
        columns: vec![
            tr(zh, "File", "文件").into(),
            tr(zh, "Detected errors", "检测到的错误").into(),
            tr(zh, "Detected warnings", "检测到的警告").into(),
            tr(zh, "Status", "状态").into(),
        ],
        rows: vec![],
    };
    let (mut total_bytes, mut errors, mut warnings, mut limited) = (0usize, 0usize, 0usize, 0usize);
    let mut names = BTreeSet::new();
    let mut groups: Vec<VecDeque<Finding>> = Vec::new();
    for file in files {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let name = file["relative_name"].as_str().ok_or(Failure::Unsupported)?;
        let text = file["text"].as_str().ok_or(Failure::Unsupported)?;
        let media = file["media_type"].as_str().ok_or(Failure::Unsupported)?;
        if !relative_name(name)
            || !names.insert(name.to_lowercase())
            || !matches!(
                media,
                "application/json"
                    | "application/toml"
                    | "text/x-java-properties"
                    | "application/yaml"
                    | "text/plain"
            )
        {
            return Err(Failure::Unsupported);
        }
        let expected_media = match name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("json") => "application/json",
            Some("toml") => "application/toml",
            Some("properties") => "text/x-java-properties",
            Some("yaml" | "yml") => "application/yaml",
            Some("txt") => "text/plain",
            _ => return Err(Failure::Unsupported),
        };
        if media != expected_media {
            return Err(Failure::Unsupported);
        }
        total_bytes += text.len();
        if text.len() > 256 * 1024 || total_bytes > 2 * 1024 * 1024 {
            return Err(Failure::Limit);
        }
        let report =
            analyze_bytes_with_options(text.as_bytes(), media, format, locale, warn_duplicates)?;
        let file_errors = report
            .findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Error))
            .count();
        let file_warnings = report
            .findings
            .iter()
            .filter(|f| matches!(f.severity, Severity::Warning))
            .count();
        let has_limit = report.findings.iter().any(|f| f.code == "finding_limit");
        let unknown = report.findings.iter().any(|f| f.code == "format_required");
        errors += file_errors;
        warnings += file_warnings;
        limited += usize::from(has_limit);
        let status = if has_limit {
            tr(zh, "More issues may remain", "可能还有其他问题")
        } else if unknown {
            tr(zh, "Choose text format", "请选择文本格式")
        } else if file_errors + file_warnings > 0 {
            tr(zh, "Issues detected", "检测到问题")
        } else {
            tr(zh, "No issues detected", "未发现问题")
        };
        file_table.rows.push(vec![
            name.into(),
            file_errors.to_string(),
            file_warnings.to_string(),
            status.into(),
        ]);
        let group = report
            .findings
            .into_iter()
            .filter(|f| !matches!(f.code.as_str(), "syntax_checked" | "finding_limit"))
            .map(|mut f| {
                f.message = format!("{name} · {}", f.message);
                f.evidence.clear();
                f
            })
            .collect();
        groups.push(group);
    }
    // Round-robin gives each file a useful first detail before one noisy file
    // consumes the entire display budget. Every file retains its summary row.
    let detected_details: usize = groups.iter().map(VecDeque::len).sum();
    let mut findings = vec![];
    while findings.len() < MAX_DETAILS {
        let before = findings.len();
        for group in &mut groups {
            if let Some(finding) = group.pop_front() {
                findings.push(finding);
            }
            if findings.len() == MAX_DETAILS {
                break;
            }
        }
        if findings.len() == before {
            break;
        }
    }
    let omitted = detected_details - findings.len();
    if omitted > 0 {
        findings.push(info("folder_details_omitted",tr(zh,"Additional findings","其他诊断"),&if zh{format!("另有 {omitted} 条已检测诊断未展示。请参考各文件统计，修复后再次检查。")}else{format!("{omitted} detected findings are not shown. Use the per-file counts, fix issues and check again.")}));
    }
    findings.push(info("folder_scope",tr(zh,"Folder check","文件夹检查"),tr(zh,"Checks selected snapshot text only. Mod-specific values and compatibility were not checked; files with a per-file limit may have further issues.","仅检查所选快照中的文本，未检查模组取值和兼容性；达到单文件诊断上限的文件可能还有其他问题。")));
    let rows = [
        (
            tr(zh, "Files checked", "已检查文件"),
            files.len().to_string(),
        ),
        (
            tr(zh, "Files skipped by host", "宿主跳过的文件"),
            skipped.to_string(),
        ),
        (
            tr(zh, "Folder scan incomplete", "文件夹扫描未完成"),
            if truncated {
                tr(zh, "Yes", "是")
            } else {
                tr(zh, "No", "否")
            }
            .into(),
        ),
        (
            tr(zh, "Detected errors", "检测到的错误"),
            errors.to_string(),
        ),
        (
            tr(zh, "Detected warnings", "检测到的警告"),
            warnings.to_string(),
        ),
        (
            tr(zh, "Detected findings not shown", "未展示的已检测诊断"),
            omitted.to_string(),
        ),
        (
            tr(zh, "Files at diagnostic limit", "达到诊断上限的文件"),
            limited.to_string(),
        ),
    ]
    .into_iter()
    .map(|(label, value)| vec![label.into(), value])
    .collect();
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![
            Table {
                columns: vec![
                    tr(zh, "Summary", "汇总").into(),
                    tr(zh, "Count / state", "数量 / 状态").into(),
                ],
                rows,
            },
            file_table,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(value: &Value, locale: &str) -> Result<Report, Failure> {
        analyze(&serde_json::to_vec(value).unwrap(), "auto", locale, || {
            false
        })
    }
    fn sample() -> Value {
        serde_json::from_str(include_str!("../fixtures/folder.json")).unwrap()
    }
    fn count(report: &Report, label: &str) -> String {
        report.tables[0]
            .rows
            .iter()
            .find(|r| r[0] == label)
            .unwrap()[1]
            .clone()
    }
    #[test]
    fn groups_existing_parsers_by_name_and_preserves_privacy() {
        let report = run(&sample(), "en").unwrap();
        assert_eq!(count(&report, "Files checked"), "4");
        assert_eq!(count(&report, "Files skipped by host"), "2");
        assert_eq!(count(&report, "Detected errors"), "2");
        assert_eq!(count(&report, "Detected warnings"), "1");
        assert_eq!(report.tables[1].rows.len(), 4);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.message.starts_with("graphics.json · Line 3"))
        );
        let output = format!("{report:?}");
        for secret in ["private-token", "private-world", "enabled=false"] {
            assert!(!output.contains(secret));
        }
        assert!(report.findings.iter().all(|f| f.evidence.is_empty()));
    }
    #[test]
    fn global_budget_is_fair_and_omission_counts_are_explicit() {
        let files:Vec<_>=(0..64).map(|n|serde_json::json!({"relative_name":format!("file{n}.properties"),"media_type":"text/x-java-properties","text":"key=one\nkey=two\nkey=three"})).collect();
        let report = run(
            &serde_json::json!({"schema_version":1,"files":files,"skipped":5,"truncated":true}),
            "en",
        )
        .unwrap();
        assert_eq!(count(&report, "Detected warnings"), "128");
        assert_eq!(count(&report, "Detected findings not shown"), "68");
        assert_eq!(count(&report, "Folder scan incomplete"), "Yes");
        assert_eq!(report.tables[1].rows.len(), 64);
        assert_eq!(report.findings.len(), 62);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.message.starts_with("file59.properties"))
        );
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "folder_details_omitted")
        );
    }
    #[test]
    fn per_file_limits_and_empty_folder_are_honest() {
        let report=run(&serde_json::json!({"schema_version":1,"files":[{"relative_name":"many.properties","media_type":"text/x-java-properties","text":"key=one\n".repeat(100)}],"skipped":0,"truncated":false}),"en").unwrap();
        assert_eq!(count(&report, "Files at diagnostic limit"), "1");
        assert!(report.tables[1].rows[0][3].contains("More issues"));
        let report = run(
            &serde_json::json!({"schema_version":1,"files":[],"skipped":7,"truncated":false}),
            "zh-CN",
        )
        .unwrap();
        assert_eq!(report.tables[1].rows.len(), 0);
        assert_eq!(count(&report, "已检查文件"), "0");
        assert_eq!(count(&report, "宿主跳过的文件"), "7");
    }
    #[test]
    fn rejects_invalid_snapshot_paths_media_versions_and_limits() {
        for name in [
            "../secret.json",
            "/secret.json",
            "C:/secret.json",
            "dir\\secret.json",
        ] {
            let mut v = sample();
            v["files"][0]["relative_name"] = name.into();
            assert!(run(&v, "en").is_err());
        }
        let mut v = sample();
        v["schema_version"] = 2.into();
        assert!(run(&v, "en").is_err());
        let mut v = sample();
        v["files"][0]["media_type"] = "application/toml".into();
        assert!(run(&v, "en").is_err());
        let mut v = sample();
        v["files"].as_array_mut().unwrap().push(serde_json::json!({"relative_name":"GRAPHICS.JSON","media_type":"application/json","text":"{}"}));
        assert!(run(&v, "en").is_err());
        let mut v = sample();
        v["files"][0]["text"] = " ".repeat(256 * 1024 + 1).into();
        assert!(matches!(run(&v, "en"), Err(Failure::Limit)));
        assert!(matches!(
            analyze(
                include_bytes!("../fixtures/folder.json"),
                "auto",
                "en",
                || true
            ),
            Err(Failure::Cancelled)
        ));
    }
}

#[cfg(test)]
mod setting_tests {
    use super::*;
    #[test]
    fn folder_totals_and_details_respect_the_duplicate_warning_preference() {
        let bytes = include_bytes!("../fixtures/folder.json");
        let report = analyze_with_options(bytes, "auto", "en", || false, false).unwrap();
        let warnings = report.tables[0]
            .rows
            .iter()
            .find(|row| row[0] == "Detected warnings")
            .unwrap();
        assert_eq!(warnings[1], "0");
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.code == "properties_duplicate")
        );
        assert_eq!(
            report
                .findings
                .iter()
                .filter(|finding| matches!(finding.severity, Severity::Error))
                .count(),
            2
        );
        assert_eq!(report.tables[1].rows[2][3], "No issues detected");
    }
}

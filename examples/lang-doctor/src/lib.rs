// SPDX-License-Identifier: MIT
//! Read-only Java language JSON comparison over one explicit host resource grant.
// Only generated component bindings require unsafe ABI shims.
#[allow(unsafe_code)]
mod bindings {
    wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
    use super::LanguageDoctor;
    export!(LanguageDoctor with_types_in self);
}
use bindings::{exports, launcher};
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde::{
    Deserialize,
    de::{self, MapAccess, Visitor},
};
use std::collections::{BTreeMap, BTreeSet};
const LIMIT: usize = 2047;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    schema_version: u32,
    protocol: String,
    source_name: String,
    pack_format: Option<i32>,
    namespace: Option<String>,
    baseline_locale: Option<String>,
    target_locale: String,
    files: Vec<Document>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    id: String,
    name: String,
    locale: String,
    text: String,
}
struct Entries(Vec<(String, String)>);
impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Entries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object of translation strings")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Entries, M::Error> {
                let mut entries = Vec::new();
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    if entries.len() >= 20_000 {
                        return Err(de::Error::custom("Language key limit exceeded"));
                    }
                    entries.push((key, value));
                }
                Ok(Entries(entries))
            }
        }
        d.deserialize_map(V)
    }
}
fn short(value: &str) -> String {
    value.chars().take(400).collect()
}
fn key_lines(text: &str) -> BTreeMap<String, Vec<usize>> {
    let bytes = text.as_bytes();
    let newlines: Vec<_> = bytes
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| (*byte == b'\n').then_some(index))
        .collect();
    let mut index = 0;
    let mut lines = BTreeMap::<String, Vec<usize>>::new();
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < bytes.len() {
            if bytes[index] == b'\\' {
                index = (index + 2).min(bytes.len());
            } else if bytes[index] == b'"' {
                index += 1;
                break;
            } else {
                index += 1;
            }
        }
        let end = index;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) == Some(&b':') {
            if let Ok(key) = serde_json::from_str::<String>(&text[start..end]) {
                let line = newlines.partition_point(|&index| index < start) + 1;
                lines.entry(key).or_default().push(line);
            }
        }
    }
    lines
}
fn placeholders(text: &str) -> BTreeMap<String, usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut next = 1;
    let mut found = BTreeMap::new();
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        i += 1;
        if bytes.get(i) == Some(&b'%') {
            i += 1;
            continue;
        }
        let start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let explicit = if bytes.get(i) == Some(&b'$') && i > start {
            let value = text[start..i].parse::<usize>().ok();
            i += 1;
            value
        } else {
            i = start;
            None
        };
        while bytes.get(i).is_some_and(|c| b"-#+ 0,(".contains(c)) {
            i += 1;
        }
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if bytes.get(i) == Some(&b'.') {
            i += 1;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        let Some(&kind) = bytes.get(i) else { break };
        i += 1;
        if b"bBhHsScCdoxXeEfgGaA".contains(&kind) {
            let argument = explicit.unwrap_or_else(|| {
                let n = next;
                next += 1;
                n
            });
            *found
                .entry(format!(
                    "{argument}:{}",
                    char::from(kind).to_ascii_lowercase()
                ))
                .or_default() += 1;
        }
    }
    found
}
fn row(
    kind: &str,
    key: &str,
    line: usize,
    reference: &str,
    translation: &str,
    message: &str,
    file: &str,
) -> Vec<String> {
    vec![
        kind.into(),
        short(key),
        line.to_string(),
        short(reference),
        short(translation),
        short(message),
        file.into(),
    ]
}
fn parse(
    document: &Document,
    issues: &mut Vec<Vec<String>>,
) -> Option<BTreeMap<String, (String, usize)>> {
    let entries = match serde_json::from_str::<Entries>(&document.text) {
        Ok(entries) => entries.0,
        Err(error) => {
            issues.push(row(
                if error.is_data() {
                    "invalid_language"
                } else {
                    "invalid_json"
                },
                "",
                error.line(),
                "",
                "",
                &error.to_string(),
                &document.id,
            ));
            return None;
        }
    };
    let locations = key_lines(&document.text);
    let mut result = BTreeMap::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for (key, value) in entries {
        let count = counts.entry(key.clone()).or_default();
        let line = locations
            .get(&key)
            .and_then(|lines| lines.get(*count))
            .copied()
            .unwrap_or(1);
        *count += 1;
        if result.contains_key(&key) {
            issues.push(row(
                "duplicate_key",
                &key,
                line,
                "",
                &value,
                "Repeated JSON key",
                &document.id,
            ));
        }
        result.insert(key, (value, line));
    }
    Some(result)
}
pub fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(Failure::Limit);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if envelope.schema_version != 1
        || envelope.protocol != "cobble.lang-package@1"
        || envelope.files.is_empty()
        || envelope.files.len() > 2
        || envelope
            .files
            .iter()
            .any(|f| f.text.len() > 1024 * 1024 || f.id.len() > 80 || f.name.len() > 512)
    {
        return Err(Failure::Unsupported);
    }
    let mut issues = Vec::new();
    let target = envelope
        .files
        .iter()
        .find(|file| file.locale == envelope.target_locale)
        .ok_or(Failure::Unsupported)?;
    let target_values = parse(target, &mut issues);
    let baseline = envelope
        .baseline_locale
        .as_ref()
        .and_then(|value| envelope.files.iter().find(|file| &file.locale == value));
    let baseline_values = baseline
        .filter(|file| file.id != target.id)
        .and_then(|file| parse(file, &mut issues));
    if let (Some(reference), Some(translations)) = (&baseline_values, &target_values) {
        for (key, (value, line)) in reference {
            match translations.get(key) {
                None => issues.push(row(
                    "missing_key",
                    key,
                    *line,
                    value,
                    "",
                    "Missing from target language",
                    &baseline.unwrap().id,
                )),
                Some((translated, target_line)) => {
                    if placeholders(value) != placeholders(translated) {
                        issues.push(row(
                            "placeholder_mismatch",
                            key,
                            *target_line,
                            value,
                            translated,
                            "Java-style format arguments differ",
                            &target.id,
                        ));
                    }
                }
            }
        }
        for (key, (value, line)) in translations {
            if !reference.contains_key(key) {
                issues.push(row(
                    "extra_key",
                    key,
                    *line,
                    "",
                    value,
                    "Not in the selected baseline",
                    &target.id,
                ));
            }
        }
    }
    let total = issues.len();
    let mut kinds = BTreeSet::new();
    for issue in &issues {
        kinds.insert(issue[0].clone());
    }
    issues.truncate(LIMIT);
    let mut row_bytes = 0;
    let mut visible = 0;
    for issue in &issues {
        let bytes = serde_json::to_vec(issue).map_err(|_| Failure::Limit)?.len();
        if row_bytes + bytes > 1536 * 1024 {
            break;
        }
        row_bytes += bytes;
        visible += 1;
    }
    issues.truncate(visible);
    let zh = locale == "zh-CN";
    let message = if total == 0 {
        if zh {
            "所选文件未发现问题。"
        } else {
            "No issues found in the selected files."
        }
    } else if zh {
        "在下方查看问题及其源文件行号。"
    } else {
        "Review issues and source lines below."
    };
    let mut findings = vec![Finding {
        code: "language_summary".into(),
        severity: if kinds.contains("invalid_json") || kinds.contains("invalid_language") {
            Severity::Error
        } else if total > 0 {
            Severity::Warning
        } else {
            Severity::Info
        },
        title: if zh { "语言检查" } else { "Language check" }.into(),
        message: message.into(),
        evidence: vec![],
        suggestions: vec![],
    }];
    if total > visible {
        findings.push(Finding {
            code: "language_rows_truncated".into(),
            severity: Severity::Warning,
            title: if zh {
                "结果已截断"
            } else {
                "Results truncated"
            }
            .into(),
            message: format!("{visible} / {total}"),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![
            Table {
                columns: [
                    "source",
                    "namespace",
                    "baseline",
                    "target",
                    "pack_format",
                    "reference_keys",
                    "target_keys",
                    "issue_count",
                ]
                .map(str::to_owned)
                .to_vec(),
                rows: vec![vec![
                    short(&envelope.source_name),
                    envelope.namespace.unwrap_or_default(),
                    envelope.baseline_locale.unwrap_or_default(),
                    envelope.target_locale,
                    envelope
                        .pack_format
                        .map(|v| v.to_string())
                        .unwrap_or_default(),
                    baseline_values
                        .as_ref()
                        .map_or(0, BTreeMap::len)
                        .to_string(),
                    target_values.as_ref().map_or(0, BTreeMap::len).to_string(),
                    total.to_string(),
                ]],
            },
            Table {
                columns: [
                    "kind",
                    "key",
                    "line",
                    "reference",
                    "translation",
                    "message",
                    "file",
                ]
                .map(str::to_owned)
                .to_vec(),
                rows: issues,
            },
        ],
    })
}
struct LanguageDoctor;
impl exports::launcher::resource_report::reports::Guest for LanguageDoctor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let metadata = input.describe()?;
        if metadata.media_type != "application/json" || metadata.byte_length > 8 * 1024 * 1024 {
            return Err(Failure::Unsupported);
        }
        let mut bytes = Vec::with_capacity(metadata.byte_length as usize);
        while bytes.len() < metadata.byte_length as usize {
            if resources::cancelled() {
                return Err(Failure::Cancelled);
            }
            let chunk = input.read_at(
                bytes.len() as u64,
                ((metadata.byte_length as usize - bytes.len()).min(65536)) as u32,
            )?;
            if chunk.is_empty() {
                return Err(Failure::Io);
            }
            bytes.extend(chunk);
        }
        analyze_bytes(&bytes, &launcher::report_context::context::get().locale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(reference: &str, target: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"protocol":"cobble.lang-package@1","sourceName":"original-fixture.zip","packFormat":34,"namespace":"example","baselineLocale":"en_us","targetLocale":"zh_cn","files":[{"id":"reference","name":"assets/example/lang/en_us.json","locale":"en_us","text":reference},{"id":"target","name":"assets/example/lang/zh_cn.json","locale":"zh_cn","text":target}]})).unwrap()
    }
    #[test]
    fn missing_placeholders_and_duplicate_keys_have_real_source_lines() {
        let bytes = input(
            "{\n\"hello\":\"Hello %s\",\n\"missing\":\"Reference\"\n}",
            "{\n\"hello\":\"你好\",\n\"repeat\":\"One\",\n\"repeat\":\"Two\"\n}",
        );
        let report = analyze_bytes(&bytes, "en").unwrap();
        let rows = &report.tables[1].rows;
        assert!(rows.iter().any(|r| r[0] == "placeholder_mismatch"
            && r[1] == "hello"
            && r[2] == "2"
            && r[6] == "target"));
        assert!(
            rows.iter()
                .any(|r| r[0] == "missing_key" && r[1] == "missing" && r[6] == "reference")
        );
        assert!(
            rows.iter()
                .any(|r| r[0] == "duplicate_key" && r[1] == "repeat" && r[2] == "4")
        );
    }
    #[test]
    fn invalid_json_and_nonstring_values_are_distinguished() {
        for (target, kind) in [
            ("{\n\"broken\":", "invalid_json"),
            ("{\"number\":42}", "invalid_language"),
        ] {
            let report = analyze_bytes(&input("{}", target), "zh-CN").unwrap();
            assert!(report.tables[1].rows.iter().any(|r| r[0] == kind));
        }
    }
    #[test]
    fn printf_positions_counts_and_escaped_percent_are_compared_without_formatting() {
        assert_eq!(placeholders("Hello %s %% %d"), placeholders("%2$d %1$s %%"));
        assert_ne!(placeholders("%1$s %1$s"), placeholders("%1$s %2$s"));
        assert_ne!(placeholders("%1$d"), placeholders("%1$s"));
        assert!(placeholders("100%%").is_empty());
        let lines = key_lines("{\n\"escaped\\\"key\" : \"text\",\n\"other\":\"value\"}");
        assert_eq!(lines["escaped\"key"], [2]);
    }
    #[test]
    fn large_reports_leave_room_for_summary_and_disclose_truncation() {
        let reference: serde_json::Map<String, serde_json::Value> = (0..2100)
            .map(|i| {
                (
                    format!("translation.{i}"),
                    serde_json::Value::String("Value".into()),
                )
            })
            .collect();
        let report = analyze_bytes(
            &input(&serde_json::to_string(&reference).unwrap(), "{}"),
            "en",
        )
        .unwrap();
        assert!(
            report
                .tables
                .iter()
                .map(|table| table.rows.len())
                .sum::<usize>()
                <= 2048
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "language_rows_truncated")
        );
        assert_eq!(report.tables[0].rows[0][7], "2100");
    }
}

// SPDX-License-Identifier: MIT
//! Read-only syntax diagnostics; no Minecraft schema or compatibility inference.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity};
use launcher::resource_report::resources::{self, Failure, Input};
use std::collections::BTreeMap;
mod folder;
const MAX_BYTES: usize = 1024 * 1024;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn issue(code: &str, warning: bool, line: usize, column: usize, advice: &str, zh: bool) -> Finding {
    Finding {
        code: code.into(),
        severity: if warning {
            Severity::Warning
        } else {
            Severity::Error
        },
        title: tr(zh, "Configuration issue", "配置问题").into(),
        message: if zh {
            format!("第 {line} 行，第 {column} 列")
        } else {
            format!("Line {line}, column {column}")
        },
        evidence: vec![],
        suggestions: vec![advice.into()],
    }
}
fn info(code: &str, title: &str, message: &str) -> Finding {
    Finding {
        code: code.into(),
        severity: Severity::Info,
        title: title.into(),
        message: message.into(),
        evidence: vec![],
        suggestions: vec![],
    }
}
fn decode(raw: &str) -> Option<Vec<u16>> {
    let mut units = Vec::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        let c = if c == '\\' {
            match chars.next()? {
                'u' => {
                    let mut n = 0u16;
                    for _ in 0..4 {
                        n = (n << 4) | chars.next()?.to_digit(16)? as u16;
                    }
                    units.push(n);
                    continue;
                }
                't' => '\t',
                'r' => '\r',
                'n' => '\n',
                'f' => '\u{c}',
                other => other,
            }
        } else {
            c
        };
        let mut buf = [0; 2];
        units.extend_from_slice(c.encode_utf16(&mut buf));
    }
    Some(units)
}
#[cfg(test)]
fn properties(text: &str, zh: bool) -> Vec<Finding> {
    properties_with_options(text, zh, true)
}
fn properties_with_options(text: &str, zh: bool, warn_duplicates: bool) -> Vec<Finding> {
    let mut findings = vec![];
    let mut keys = BTreeMap::new();
    let mut logical = String::new();
    let mut start = 1;
    for (index, line) in text.lines().chain(std::iter::once("")).enumerate() {
        if logical.is_empty()
            && line
                .trim_start_matches([' ', '\t', '\u{c}'])
                .starts_with(['#', '!'])
        {
            continue;
        }
        if logical.is_empty() {
            start = index + 1;
        }
        let part = if logical.is_empty() {
            line
        } else {
            line.trim_start_matches([' ', '\t', '\u{c}'])
        };
        logical.push_str(part);
        if logical.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1 {
            logical.pop();
            continue;
        }
        let value = logical.trim_start_matches([' ', '\t', '\u{c}']);
        if !value.is_empty() && !value.starts_with(['#', '!']) {
            let mut escaped = false;
            let mut end = value.len();
            for (i, c) in value.char_indices() {
                if !escaped && matches!(c, '=' | ':' | ' ' | '\t' | '\u{c}') {
                    end = i;
                    break;
                }
                if c == '\\' {
                    escaped = !escaped;
                } else {
                    escaped = false;
                }
            }
            if decode(value).is_none() {
                findings.push(issue(
                    "properties_escape",
                    false,
                    start,
                    1,
                    tr(
                        zh,
                        "Use four hexadecimal digits after each \\u escape.",
                        "每个 \\u 后需有四位十六进制数字。",
                    ),
                    zh,
                ));
            } else if let Some(key) = decode(&value[..end])
                && let Some(first) = keys.insert(key, start)
                && warn_duplicates
            {
                let mut f = issue(
                    "properties_duplicate",
                    true,
                    start,
                    1,
                    tr(
                        zh,
                        "Remove or rename the repeated key; Java properties keeps its last value.",
                        "删除或重命名重复键；Java properties 使用最后一个值。",
                    ),
                    zh,
                );
                f.message.push_str(&if zh {
                    format!("；首次出现在第 {first} 行")
                } else {
                    format!("; first declared on line {first}")
                });
                findings.push(f);
            }
        }
        logical.clear();
        if findings.len() >= 60 {
            break;
        }
    }
    findings
}
#[cfg(test)]
fn analyze_bytes(
    bytes: &[u8],
    media: &str,
    requested: &str,
    locale: &str,
) -> Result<Report, Failure> {
    analyze_bytes_with_options(bytes, media, requested, locale, true)
}
fn analyze_bytes_with_options(
    bytes: &[u8],
    media: &str,
    requested: &str,
    locale: &str,
    warn_duplicates: bool,
) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Unsupported)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let zh = locale == "zh-CN";
    let format = if requested != "auto" && media == "text/plain" {
        requested
    } else {
        match media {
            "application/json" => "json",
            "application/toml" => "toml",
            "text/x-java-properties" => "properties",
            "application/yaml" | "text/yaml" => "yaml",
            _ if text.trim_start().starts_with(['{', '[']) => "json",
            _ => "unknown",
        }
    };
    let mut findings = vec![];
    match format {
        "json" => {
            if let Err(error) = serde_json::from_str::<serde_json::Value>(text) {
                findings.push(issue("json_syntax",false,error.line().max(1),error.column().max(1),tr(zh,"Check commas, quoted keys, values and closing brackets at this position. JSON does not allow comments or trailing commas.","检查此位置的逗号、带引号的键、值和结束括号。JSON 不支持注释和尾随逗号。"),zh));
            }
        }
        "toml" => {
            if let Err(error) = text.parse::<toml::Table>() {
                let offset = error.span().map_or(0, |s| s.start).min(text.len());
                let prefix = &text[..offset];
                let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
                let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
                findings.push(issue("toml_syntax",false,line,column,tr(zh,"Check the assignment, value type, quotes, table declaration or repeated key at this position.","检查此位置的赋值、值类型、引号、表声明或重复键。"),zh));
            }
        }
        "yaml" => {
            let mut budget = serde_saphyr::Budget::default();
            budget.max_depth = 32;
            budget.flow_nesting_limit = 32;
            budget.max_events = 100_000;
            budget.max_aliases = 0;
            budget.max_anchors = 0;
            let mut options = serde_saphyr::Options::default();
            options.budget = Some(budget);
            options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
            options.merge_keys = serde_saphyr::MergeKeyPolicy::Error;
            options.reject_unsupported_tags = true;
            if let Err(error) = serde_saphyr::from_str_with_options::<serde_json::Value>(text, options) {
                let (line, column) = error.location().map_or((1, 1), |l| (l.line() as usize, l.column() as usize));
                findings.push(issue("yaml_syntax_or_scope", false, line, column, tr(zh,"Check indentation, mapping keys and values. This checker supports one JSON-compatible YAML document, without anchors, aliases, merge keys or custom tags, up to depth 32.","检查缩进、映射键和值。此检查器支持单个与 JSON 兼容的 YAML 文档，不支持锚点、别名、合并键和自定义标签，最多嵌套 32 层。"),zh));
            }
        }
        "properties" => findings = properties_with_options(text, zh, warn_duplicates),
        _ => findings.push(info(
            "format_required",
            tr(zh, "Choose a format", "请选择格式"),
            tr(
                zh,
                "Choose JSON, TOML, YAML or Java properties in extension settings for this text file.",
                "请在扩展设置中为此文本文件选择 JSON、TOML、YAML 或 Java properties。",
            ),
        )),
    }
    if findings.len() >= 60 {
        findings.push(info(
            "finding_limit",
            tr(zh, "More issues may remain", "可能还有其他问题"),
            tr(
                zh,
                "Fix these issues and check again.",
                "修复这些问题后请再次检查。",
            ),
        ));
    }
    if findings.is_empty() {
        findings.push(info(
            "syntax_checked",
            tr(zh, "No syntax issues found", "未发现语法问题"),
            tr(
                zh,
                "Mod-specific values and compatibility were not checked.",
                "未检查模组取值和兼容性。",
            ),
        ));
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![],
    })
}
struct ConfigDoctor;
impl exports::launcher::resource_report::reports::Guest for ConfigDoctor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let metadata = input.describe()?;
        let maximum = if metadata.media_type == folder::MEDIA_TYPE {
            folder::MAX_ENVELOPE_BYTES
        } else {
            MAX_BYTES
        };
        if metadata.byte_length > maximum as u64 {
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
        let context = launcher::report_context::context::get();
        let settings: serde_json::Value =
            serde_json::from_str(&context.settings_json).map_err(|_| Failure::Unsupported)?;
        let report = if metadata.media_type == folder::MEDIA_TYPE {
            folder::analyze_with_options(
                &bytes,
                settings["format"].as_str().unwrap_or("auto"),
                &context.locale,
                resources::cancelled,
                settings["warn_duplicate_properties"]
                    .as_bool()
                    .unwrap_or(true),
            )?
        } else {
            analyze_bytes_with_options(
                &bytes,
                &metadata.media_type,
                settings["format"].as_str().unwrap_or("auto"),
                &context.locale,
                settings["warn_duplicate_properties"]
                    .as_bool()
                    .unwrap_or(true),
            )?
        };
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(ConfigDoctor);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_error_never_echoes_values() {
        let r = analyze_bytes(
            b"{\n\"secret-token\":true,\n}",
            "application/json",
            "auto",
            "en",
        )
        .unwrap();
        assert_eq!(r.findings[0].code, "json_syntax");
        assert!(r.findings[0].message.contains('3'));
        assert!(!format!("{r:?}").contains("secret-token"));
    }
    #[test]
    fn toml_duplicate_and_valid() {
        let r = analyze_bytes(
            b"enabled=true\nenabled=false",
            "application/toml",
            "auto",
            "zh-CN",
        )
        .unwrap();
        assert_eq!(r.findings[0].code, "toml_syntax");
        assert!(r.findings[0].message.contains('2'));
        assert_eq!(
            analyze_bytes(
                b"[graphics]\nenabled=true",
                "application/toml",
                "auto",
                "en"
            )
            .unwrap()
            .findings[0]
                .code,
            "syntax_checked"
        );
    }
    #[test]
    fn properties_handles_escaped_keys_and_continuations() {
        let r = properties(
            "# comment\nhello\\ world=one\nhello\\u0020world:two\njoined\\\n key=yes\njoinedkey=no\nbad=\\u12ZZ",
            false,
        );
        assert_eq!(
            r.iter().map(|f| f.code.as_str()).collect::<Vec<_>>(),
            [
                "properties_duplicate",
                "properties_duplicate",
                "properties_escape"
            ]
        );
        assert!(r[1].message.contains('6'));
        assert!(properties("key=hello\\z", false).is_empty());
        assert!(properties("key=\\uD800", false).is_empty());
        assert_eq!(
            analyze_bytes(b"{}", "application/json", "toml", "en")
                .unwrap()
                .findings[0]
                .code,
            "syntax_checked"
        );
    }
    #[test]
    fn bounded_and_explicit_format() {
        assert!(matches!(
            analyze_bytes(&vec![b'a'; MAX_BYTES + 1], "text/plain", "auto", "en"),
            Err(Failure::Limit)
        ));
        assert!(matches!(
            analyze_bytes(&[255], "text/plain", "auto", "en"),
            Err(Failure::Unsupported)
        ));
        assert_eq!(
            analyze_bytes(b"a: b", "text/plain", "auto", "en")
                .unwrap()
                .findings[0]
                .code,
            "format_required"
        );
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    #[test]
    fn original_format_fixtures() {
        for (bytes, media, code) in [
            (
                include_bytes!("../fixtures/invalid.json").as_slice(),
                "application/json",
                "json_syntax",
            ),
            (
                include_bytes!("../fixtures/duplicate.toml").as_slice(),
                "application/toml",
                "toml_syntax",
            ),
            (
                include_bytes!("../fixtures/duplicate.properties").as_slice(),
                "text/x-java-properties",
                "properties_duplicate",
            ),
            (
                include_bytes!("../fixtures/invalid.yaml").as_slice(),
                "application/yaml",
                "yaml_syntax_or_scope",
            ),
        ] {
            assert_eq!(
                analyze_bytes(bytes, media, "auto", "en").unwrap().findings[0].code,
                code
            );
        }
    }
    #[test]
    fn yaml_is_bounded_and_rejects_duplicate_alias_and_tags() {
        for text in [
            "key: 1\nkey: 2",
            "key: &anchor 1\nother: *anchor",
            "key: !external anything",
        ] {
            let r = analyze_bytes(text.as_bytes(), "application/yaml", "auto", "zh-CN").unwrap();
            assert_eq!(r.findings[0].code, "yaml_syntax_or_scope");
            assert!(!r.findings[0].suggestions[0].is_empty());
        }
        assert_eq!(
            analyze_bytes(
                b"graphics:\n  enabled: true",
                "application/yaml",
                "auto",
                "en"
            )
            .unwrap()
            .findings[0]
                .code,
            "syntax_checked"
        );
        let deep = format!("{}0{}", "[".repeat(40), "]".repeat(40));
        assert_eq!(
            analyze_bytes(deep.as_bytes(), "application/yaml", "auto", "en")
                .unwrap()
                .findings[0]
                .code,
            "yaml_syntax_or_scope"
        );
    }
    #[test]
    fn properties_comment_continuation_and_finding_bound() {
        assert!(properties("# comment\\\nkey=yes", false).is_empty());
        assert_eq!(
            properties("key=1\nkey=2\\", false)[0].code,
            "properties_duplicate"
        );
        let text = "key=1\n".repeat(100);
        assert_eq!(
            analyze_bytes(text.as_bytes(), "text/x-java-properties", "auto", "en")
                .unwrap()
                .findings
                .len(),
            61
        );
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    #[test]
    fn disabling_duplicate_warnings_keeps_later_escape_errors_visible() {
        let text = format!("{}bad=\\u12ZZ", "same=one\nsame=two\n".repeat(100));
        let report = analyze_bytes_with_options(
            text.as_bytes(),
            "text/x-java-properties",
            "auto",
            "en",
            false,
        )
        .unwrap();
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "properties_escape");
        assert!(report.findings[0].message.contains("201"));
        let report = analyze_bytes_with_options(
            b"same=one\nsame=two",
            "text/x-java-properties",
            "auto",
            "en",
            true,
        )
        .unwrap();
        assert_eq!(report.findings[0].code, "properties_duplicate");
    }
    #[test]
    fn format_and_other_syntax_errors_are_independent_of_duplicate_setting() {
        let report = analyze_bytes_with_options(
            b"same=one\nsame=two",
            "text/plain",
            "properties",
            "zh-CN",
            false,
        )
        .unwrap();
        assert_eq!(report.findings[0].code, "syntax_checked");
        for (input, media, code) in [
            (b"{".as_slice(), "application/json", "json_syntax"),
            (b"key=".as_slice(), "application/toml", "toml_syntax"),
        ] {
            assert_eq!(
                analyze_bytes_with_options(input, media, "auto", "en", false)
                    .unwrap()
                    .findings[0]
                    .code,
                code
            );
        }
    }
}

#[cfg(test)]
mod manifest_tests {
    #[test]
    fn standard_form_retains_existing_format_values_and_declares_guest_setting_keys() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../plugin.json")).unwrap();
        let settings = &manifest["contributions"]["settings"];
        assert!(settings.get("categories").is_none());
        let form = &settings["form"];
        assert_eq!(form["scope"], "global");
        let schema = &form["schema"];
        assert_eq!(schema["properties"]["format"]["default"], "auto");
        assert_eq!(
            schema["properties"]["format"]["enum"],
            serde_json::json!(["auto", "json", "toml", "properties", "yaml"])
        );
        assert_eq!(
            schema["properties"]["warn_duplicate_properties"]["default"],
            true
        );
        assert_eq!(
            schema["properties"]["warn_duplicate_properties"]["type"],
            "boolean"
        );
        let permission = manifest["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["permission"] == "launcher:settings/read")
            .unwrap();
        assert_eq!(
            permission["keys"],
            serde_json::json!(["global:format", "global:warn_duplicate_properties"])
        );
    }
}

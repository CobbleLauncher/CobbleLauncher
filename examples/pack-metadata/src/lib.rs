// SPDX-License-Identifier: MIT
//! Original bounded read-only pack metadata report, without game-version mapping.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
const MAX_BYTES: usize = 1024 * 1024;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn finding(code: &str, warning: bool, path: &str, advice: &str, zh: bool) -> Finding {
    Finding {
        code: code.into(),
        severity: if warning {
            Severity::Warning
        } else {
            Severity::Error
        },
        title: tr(zh, "Check metadata", "检查元数据").into(),
        message: path.into(),
        evidence: vec![],
        suggestions: vec![advice.into()],
    }
}
fn number(v: &Value) -> Option<u32> {
    v.as_u64()
        .filter(|n| *n <= i32::MAX as u64)
        .map(|n| n as u32)
}
fn range(v: &Value) -> Option<(u32, u32)> {
    if let Some(n) = number(v) {
        return Some((n, n));
    }
    let pair = if let Some(a) = v.as_array() {
        if a.len() != 2 {
            return None;
        }
        (number(&a[0])?, number(&a[1])?)
    } else {
        (
            number(v.get("min_inclusive")?)?,
            number(v.get("max_inclusive")?)?,
        )
    };
    (pair.0 <= pair.1).then_some(pair)
}
fn modern(v: &Value, maximum: bool) -> Option<(u32, u32)> {
    if let Some(n) = number(v) {
        return Some((n, if maximum { i32::MAX as u32 } else { 0 }));
    }
    let a = v.as_array()?;
    if a.is_empty() || a.len() > 2 {
        return None;
    }
    Some((
        number(&a[0])?,
        if a.len() == 2 {
            number(&a[1])?
        } else if maximum {
            i32::MAX as u32
        } else {
            0
        },
    ))
}
fn preview(text: &str) -> String {
    let mut p: String = text
        .chars()
        .take(400)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.chars().count() > 400 {
        p.push('…');
    }
    p
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let zh = locale == "zh-CN";
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Unsupported)?;
    let mut report = Report {
        schema_version: 1,
        findings: vec![],
        tables: vec![Table {
            columns: vec![
                tr(zh, "Field", "字段").into(),
                tr(zh, "Declared metadata", "声明的元数据").into(),
            ],
            rows: vec![],
        }],
    };
    let value: Value = match serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text)) {
        Ok(v) => v,
        Err(e) => {
            report.findings.push(finding(
                "metadata_json",
                false,
                &format!(
                    "{} {}, {} {}",
                    tr(zh, "Line", "行"),
                    e.line(),
                    tr(zh, "column", "列"),
                    e.column()
                ),
                tr(
                    zh,
                    "Correct JSON punctuation and quotes at this position, then inspect again.",
                    "修正此位置的 JSON 标点和引号，然后再次检查。",
                ),
                zh,
            ));
            return Ok(report);
        }
    };
    let Some(root) = value.as_object() else {
        report.findings.push(finding(
            "metadata_root",
            false,
            "$",
            tr(
                zh,
                "Use a JSON object containing a pack object.",
                "使用包含 pack 对象的 JSON 对象。",
            ),
            zh,
        ));
        return Ok(report);
    };
    let Some(pack) = root.get("pack").and_then(Value::as_object) else {
        report.findings.push(finding("metadata_pack",false,"pack",tr(zh,"Add a pack object with description and format metadata; select the root pack.mcmeta file.","添加包含 description 和格式声明的 pack 对象；请选择根目录的 pack.mcmeta 文件。"),zh));
        return Ok(report);
    };
    match pack.get("description"){
        None=>report.findings.push(finding("description_missing",false,"pack.description",tr(zh,"Add a pack description string or text component.","添加资源包描述字符串或文本组件。"),zh)),
        Some(Value::String(s))=>{report.tables[0].rows.push(vec![tr(zh,"Description preview","描述预览").into(),preview(s)]);if s.trim().is_empty(){report.findings.push(finding("description_empty",true,"pack.description",tr(zh,"Give the pack a short description so users can identify it.","添加简短描述，便于用户识别资源包。"),zh));}},
        Some(Value::Array(_)|Value::Object(_))=>{report.tables[0].rows.push(vec!["pack.description".into(),tr(zh,"Text component (not rendered)","文本组件（未渲染）").into()]);report.findings.push(finding("description_component_scope",true,"pack.description",tr(zh,"Inspect this text component in-game; this tool does not fully validate components or resolve translations.","请在游戏中检查此文本组件；此工具不完整验证组件，也不解析翻译。"),zh));},
        Some(_)=>report.findings.push(finding("description_scope",true,"pack.description",tr(zh,"This description shape is not inspected. Use a description string or check the component with the target game.","此描述结构未检查。请使用描述字符串，或在目标游戏中检查该组件。"),zh)),
    }
    let declared = pack.get("pack_format");
    let pf = declared.and_then(number);
    if declared.is_some() {
        if let Some(n) = pf {
            report.tables[0]
                .rows
                .push(vec!["pack.pack_format".into(), n.to_string()]);
        } else {
            report.findings.push(finding(
                "pack_format_type",
                false,
                "pack.pack_format",
                tr(
                    zh,
                    "Use a non-negative integer no larger than 2147483647.",
                    "使用不大于 2147483647 的非负整数。",
                ),
                zh,
            ));
        }
    }
    let has_modern = pack.contains_key("min_format") || pack.contains_key("max_format");
    if has_modern {
        let bounds = pack
            .get("min_format")
            .and_then(|v| modern(v, false))
            .zip(pack.get("max_format").and_then(|v| modern(v, true)));
        match bounds{Some((min,max))if min<=max=>{report.tables[0].rows.push(vec![tr(zh,"Declared format range","声明的格式范围").into(),format!("{}.{} – {}.{}",min.0,min.1,max.0,max.1)]);},_=>report.findings.push(finding("modern_range",false,"pack.min_format / pack.max_format",tr(zh,"Provide both bounds as an integer, [major], or [major, minor]; minimum must not exceed maximum.","同时提供两个边界，格式为整数、[主版本] 或 [主版本, 次版本]；下界不能大于上界。"),zh))}
    } else if declared.is_none() {
        report.findings.push(finding("format_missing",false,"pack",tr(zh,"Declare pack_format for legacy metadata, or min_format and max_format for the newer format.","为旧格式声明 pack_format，或为新格式声明 min_format 和 max_format。"),zh));
    }
    if let Some(v) = pack.get("supported_formats") {
        match range(v){Some((min,max))=>{report.tables[0].rows.push(vec!["pack.supported_formats".into(),format!("{min} – {max}")]);if let Some(n)=pf&&!(min..=max).contains(&n){report.findings.push(finding("format_outside_range",false,"pack.supported_formats",tr(zh,"Include pack_format in supported_formats, or correct the declared pack_format.","让 supported_formats 包含 pack_format，或修正声明的 pack_format。"),zh));}},None=>report.findings.push(finding("supported_formats_scope",true,"pack.supported_formats",tr(zh,"Check this range. Recognized shapes: integer, [minimum, maximum], or an object with min_inclusive and max_inclusive; minimum must not exceed maximum.","请检查此范围。可识别整数、[下界, 上界] 或同时包含 min_inclusive 和 max_inclusive 的对象；下界不能大于上界。"),zh))}
    }
    let mut uninspected = Vec::new();
    for key in root.keys() {
        if key != "pack" {
            uninspected.push(key);
        }
    }
    if !uninspected.is_empty() {
        report.tables[0].rows.push(vec![
            format!(
                "{} ({})",
                tr(zh, "Other sections (not checked)", "其他部分（未检查）"),
                uninspected.len()
            ),
            preview(
                &uninspected
                    .into_iter()
                    .take(12)
                    .map(|s| preview(s))
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
        ]);
    }
    report.findings.push(Finding{code:"metadata_scope".into(),severity:Severity::Info,title:tr(zh,"Metadata inspection","元数据检查").into(),message:tr(zh,"Declared format numbers are shown without Minecraft-version mapping. Pack assets, overlays, text-component semantics and actual game compatibility were not checked.","仅展示声明的格式编号，不映射 Minecraft 版本。未检查包内资源、覆盖层、文本组件语义和实际游戏兼容性。").into(),evidence:vec![],suggestions:vec![]});
    Ok(report)
}
struct PackMetadata;
impl exports::launcher::resource_report::reports::Guest for PackMetadata {
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
export!(PackMetadata);

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(bytes: &[u8]) -> Report {
        analyze_bytes(bytes, "en").unwrap()
    }
    #[test]
    fn legacy_range_summary_is_factual() {
        let r = fixture(include_bytes!("../fixtures/legacy.mcmeta"));
        assert_eq!(r.findings.len(), 1);
        assert_eq!(r.findings[0].code, "metadata_scope");
        assert!(
            r.tables[0]
                .rows
                .iter()
                .any(|row| row[0] == "pack.supported_formats" && row[1] == "16 – 18")
        );
        assert!(!format!("{r:?}").contains("1.20.2"));
    }
    #[test]
    fn format_must_be_in_legacy_range() {
        let r = fixture(include_bytes!("../fixtures/invalid-range.mcmeta"));
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "format_outside_range" && matches!(f.severity, Severity::Error))
        );
        for value in [
            serde_json::json!(18),
            serde_json::json!([16, 18]),
            serde_json::json!({"min_inclusive":16,"max_inclusive":18}),
        ] {
            assert!(range(&value).is_some());
        }
        assert!(range(&serde_json::json!([18, 16])).is_none());
        assert!(range(&serde_json::json!([16, 17, 18])).is_none());
    }
    #[test]
    fn modern_bounds_do_not_require_legacy_field() {
        let r = fixture(include_bytes!("../fixtures/modern.mcmeta"));
        assert!(
            !r.findings
                .iter()
                .any(|f| matches!(f.severity, Severity::Error))
        );
        assert!(
            r.tables[0]
                .rows
                .iter()
                .any(|row| row[1] == "74.1 – 74.2147483647")
        );
        assert_eq!(modern(&serde_json::json!([74]), false), Some((74, 0)));
        assert_eq!(
            modern(&serde_json::json!(74), true),
            Some((74, i32::MAX as u32))
        );
        let bad =
            fixture(br#"{"pack":{"description":"x","min_format":[74,2],"max_format":[74,1]}}"#);
        assert!(bad.findings.iter().any(|f| f.code == "modern_range"));
    }
    #[test]
    fn actionable_missing_fields_and_uninspected_components() {
        let r = fixture(include_bytes!("../fixtures/missing-description.mcmeta"));
        assert!(r.findings.iter().any(|f| f.code == "description_missing"));
        assert!(r.findings.iter().any(|f| f.code == "pack_format_type"));
        let r = fixture(br#"{"pack":{"description":[{"translate":"key"}],"pack_format":18}}"#);
        assert!(
            r.findings
                .iter()
                .any(|f| f.code == "description_component_scope")
        );
        let r = fixture(include_bytes!("../fixtures/unknown-sections.mcmeta"));
        assert!(r.tables[0].rows.iter().any(|row| row[1] == "overlays"));
    }
    #[test]
    fn malformed_unrelated_and_limits() {
        assert_eq!(fixture(b"{broken").findings[0].code, "metadata_json");
        assert_eq!(fixture(b"[]").findings[0].code, "metadata_root");
        assert_eq!(fixture(b"{}").findings[0].code, "metadata_pack");
        assert!(matches!(
            analyze_bytes(&vec![b' '; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
        assert!(matches!(
            analyze_bytes(&[255], "en"),
            Err(Failure::Unsupported)
        ));
        let p = preview(&"中".repeat(1000));
        assert_eq!(p.chars().count(), 401);
        assert!(p.ends_with('…'));
    }
    #[test]
    fn summaries_are_bounded_for_large_unicode_metadata() {
        let mut root = serde_json::Map::new();
        root.insert(
            "pack".into(),
            serde_json::json!({"description":"中".repeat(10000),"pack_format":18}),
        );
        for n in 0..20 {
            root.insert(format!("{n}{}", "界".repeat(500)), Value::Null);
        }
        let r = fixture(&serde_json::to_vec(&root).unwrap());
        assert!(
            r.tables
                .iter()
                .flat_map(|t| &t.rows)
                .flatten()
                .all(|cell| cell.len() <= 2048)
        );
        assert!(r.tables[0].rows.last().unwrap()[0].contains("20"));
        assert!(r.tables[0].rows.last().unwrap()[1].ends_with('…'));
    }
    #[test]
    fn chinese_copy_and_bounded_summary() {
        let r = analyze_bytes(include_bytes!("../fixtures/invalid-range.mcmeta"), "zh-CN").unwrap();
        assert_eq!(r.tables[0].columns[0], "字段");
        assert!(
            r.findings
                .iter()
                .find(|f| f.code == "format_outside_range")
                .unwrap()
                .suggestions[0]
                .contains("包含")
        );
    }
}

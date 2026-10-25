// SPDX-License-Identifier: MIT
//! Bounded, read-only Minecraft statistics JSON analysis. No version inference.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRIES: usize = 20000;
const TOP: usize = 12;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    let stats = root
        .get("stats")
        .and_then(Value::as_object)
        .ok_or(Failure::Unsupported)?;
    let zh = locale == "zh-CN";
    let mut rows = Vec::new();
    let mut invalid = 0;
    let mut entries = 0;
    let mut known = false;
    for category in ["minecraft:custom", "minecraft:mined", "minecraft:killed"] {
        let Some(group) = stats.get(category) else {
            continue;
        };
        let Some(group) = group.as_object() else {
            return Err(Failure::Unsupported);
        };
        let mut ranked = Vec::new();
        for (key, value) in group {
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(Failure::Limit);
            }
            // u64 only: negative, floating and nonnumeric counters are never coerced.
            let Some(value) = value.as_u64() else {
                invalid += 1;
                continue;
            };
            if key.len() > 256 || key.chars().any(char::is_control) {
                invalid += 1;
                continue;
            }
            if category == "minecraft:custom" {
                if [
                    "minecraft:play_time",
                    "minecraft:play_one_minute",
                    "minecraft:walk_one_cm",
                    "minecraft:sprint_one_cm",
                    "minecraft:crouch_one_cm",
                    "minecraft:swim_one_cm",
                    "minecraft:fly_one_cm",
                    "minecraft:aviate_one_cm",
                    "minecraft:boat_one_cm",
                    "minecraft:minecart_one_cm",
                    "minecraft:horse_one_cm",
                    "minecraft:deaths",
                    "minecraft:mob_kills",
                    "minecraft:player_kills",
                ]
                .contains(&key.as_str())
                {
                    known = true;
                    rows.push(vec!["custom".into(), key.clone(), value.to_string()]);
                }
            } else {
                known = true;
                ranked.push((key.clone(), value));
            }
        }
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (key, value) in ranked.into_iter().take(TOP) {
            rows.push(vec![
                if category == "minecraft:mined" {
                    "mined".into()
                } else {
                    "killed".into()
                },
                key,
                value.to_string(),
            ]);
        }
    }
    if !known {
        return Err(Failure::Unsupported);
    }
    // Modern play_time takes precedence; historical alias must never be summed twice.
    if rows.iter().any(|r| r[1] == "minecraft:play_time") {
        rows.retain(|r| r[1] != "minecraft:play_one_minute");
    }
    let mut findings = vec![];
    if invalid > 0 {
        findings.push(Finding {
            code: "invalid_counters".into(),
            severity: Severity::Warning,
            title: tr(zh, "Some counters were skipped", "部分统计已跳过").into(),
            message: format!(
                "{invalid} {}",
                tr(zh, "invalid counters.", "项统计值无效。")
            ),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables: vec![Table {
            columns: vec!["category".into(), "id".into(), "value".into()],
            rows,
        }],
    })
}
struct WorldChronicle;
impl exports::launcher::resource_report::reports::Guest for WorldChronicle {
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
            let maximum = (metadata.byte_length as usize - bytes.len()).min(65536) as u32;
            let chunk = input.read_at(bytes.len() as u64, maximum)?;
            if chunk.is_empty() || chunk.len() > maximum as usize {
                return Err(Failure::Io);
            }
            bytes.extend_from_slice(&chunk);
        }
        let report = analyze_bytes(&bytes, &launcher::report_context::context::get().locale)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(WorldChronicle);
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn real_keys_and_alias_are_not_double_counted() {
        let r=analyze_bytes(br#"{"stats":{"minecraft:custom":{"minecraft:play_time":72000,"minecraft:play_one_minute":72000,"minecraft:walk_one_cm":12500},"minecraft:mined":{"minecraft:stone":55},"minecraft:killed":{"minecraft:zombie":9}}}"#,"en").unwrap();
        assert_eq!(r.tables[0].rows.len(), 4);
        assert!(
            !r.tables[0]
                .rows
                .iter()
                .any(|r| r[1] == "minecraft:play_one_minute")
        );
    }
    #[test]
    fn absent_or_unrelated_is_not_zero_playtime() {
        for input in [
            b"{}".as_slice(),
            br#"{"stats":{"minecraft:custom":{"minecraft:unknown":7}}}"#,
            br#"{"minecraft:story/root":{"done":true}}"#,
        ] {
            assert!(matches!(
                analyze_bytes(input, "en"),
                Err(Failure::Unsupported)
            ));
        }
    }
    #[test]
    fn malformed_values_are_skipped_and_top_is_bounded() {
        let mut mined = serde_json::Map::new();
        for i in 0..40 {
            mined.insert(format!("mod:block_{i}"), json!(i));
        }
        let input = json!({"stats":{"minecraft:mined":mined,"minecraft:custom":{"minecraft:deaths":-1,"minecraft:mob_kills":"9","minecraft:player_kills":0}}});
        let r = analyze_bytes(&serde_json::to_vec(&input).unwrap(), "zh-CN").unwrap();
        assert_eq!(r.tables[0].rows.len(), 13);
        assert_eq!(r.tables[0].rows[1][1], "mod:block_39");
        assert_eq!(r.findings[0].code, "invalid_counters");
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
    }
}

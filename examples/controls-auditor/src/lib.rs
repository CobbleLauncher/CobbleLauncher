// SPDX-License-Identifier: MIT
//! Read-only report over the host's allowlisted options snapshot.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});

use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_INPUT_BYTES: usize = 512 * 1024;
const MAX_OPTIONS: usize = 2_000;
const MAX_ROWS: usize = 1_000;
const UNBOUND: [&str; 2] = ["key.keyboard.unknown", "key.mouse.unknown"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    scope: String,
    game: Game,
    data: OptionsData,
    revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Game {
    id: String,
    name: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OptionsData {
    items: Vec<OptionItem>,
    truncated: bool,
    available: bool,
    #[serde(default)]
    unavailable: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionItem {
    name: String,
    value: serde_json::Value,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_snapshot(bytes: &[u8]) -> Result<Snapshot, Failure> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Failure::Limit);
    }
    let snapshot: Snapshot = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if snapshot.schema_version != 1
        || snapshot.scope != "options"
        || !valid_digest(&snapshot.revision)
        || snapshot.game.id.is_empty()
        || snapshot.game.id.len() > 256
        || snapshot.game.name.is_empty()
        || snapshot.game.name.len() > 256
        || snapshot
            .game
            .name
            .chars()
            .any(|character| character.is_control())
        || snapshot.game.version.is_empty()
        || snapshot.game.version.len() > 64
        || snapshot.data.items.len() > MAX_OPTIONS
        || (!snapshot.data.available && !snapshot.data.items.is_empty())
    {
        return Err(Failure::Unsupported);
    }
    for item in &snapshot.data.items {
        if item.name.is_empty() || item.name.len() > 128 {
            return Err(Failure::Unsupported);
        }
        if serde_json::to_vec(&item.value).map_or(usize::MAX, |bytes| bytes.len()) > 512 {
            return Err(Failure::Limit);
        }
    }
    Ok(snapshot)
}

fn key_code(value: &str) -> Option<&str> {
    value
        .strip_prefix("key.keyboard.")
        .or_else(|| value.strip_prefix("key.mouse."))
        .filter(|code| {
            !code.is_empty()
                && code.len() <= 80
                && code.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'.'
                        || byte == b'_'
                })
        })
}

fn group(name: &str, zh: bool) -> &'static str {
    if name.starts_with("key_") {
        if zh { "按键绑定" } else { "Key bindings" }
    } else if matches!(
        name,
        "rawMouseInput"
            | "mouseWheelSensitivity"
            | "discrete_mouse_scroll"
            | "invertYMouse"
            | "sensitivity"
    ) {
        if zh { "鼠标" } else { "Mouse" }
    } else if matches!(
        name,
        "autoJump" | "toggleCrouch" | "toggleSprint" | "touchscreen"
    ) {
        if zh { "操作" } else { "Gameplay" }
    } else if matches!(
        name,
        "fullscreen"
            | "fov"
            | "renderDistance"
            | "simulationDistance"
            | "guiScale"
            | "maxFps"
            | "vsync"
    ) {
        if zh { "画面" } else { "Display" }
    } else if zh {
        "其他选项"
    } else {
        "Other options"
    }
}

fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn analyze_snapshot(snapshot: Snapshot, zh: bool) -> Report {
    let mut assignments = BTreeMap::<String, BTreeSet<String>>::new();
    let mut option_name_counts = BTreeMap::<String, usize>::new();
    let mut unbound = 0usize;
    let mut unknown_key_values = 0usize;
    let mut group_counts = BTreeMap::<String, usize>::new();
    let mut option_rows = Vec::new();
    for item in &snapshot.data.items {
        *option_name_counts.entry(item.name.clone()).or_default() += 1;
        let category = group(&item.name, zh).to_owned();
        *group_counts.entry(category.clone()).or_default() += 1;
        let value = format_value(&item.value);
        if item.name.starts_with("key_") {
            if UNBOUND.contains(&value.as_str()) {
                unbound += 1;
            } else if let Some(code) = key_code(&value) {
                assignments
                    .entry(code.to_owned())
                    .or_default()
                    .insert(item.name.clone());
            } else {
                unknown_key_values += 1;
            }
        }
        if option_rows.len() < MAX_ROWS {
            option_rows.push(vec![category, item.name.clone(), value]);
        }
    }
    let duplicates: Vec<_> = assignments
        .into_iter()
        .filter(|(_, actions)| actions.len() > 1)
        .collect();
    let repeated_options: Vec<_> = option_name_counts
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .collect();
    let mut findings = Vec::new();
    if !repeated_options.is_empty() {
        let details = repeated_options
            .iter()
            .take(30)
            .map(|(name, count)| format!("{name} ({count})"))
            .collect::<Vec<_>>();
        findings.push(Finding {
            code: "repeated_option_names".into(),
            severity: Severity::Warning,
            title: if zh {
                "选项名称重复"
            } else {
                "Repeated option names"
            }
            .into(),
            message: if zh {
                format!("选项文件中有重复名称：{}", details.join("、"))
            } else {
                format!(
                    "The options file contains repeated names: {}",
                    details.join("; ")
                )
            },
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if duplicates.is_empty() {
        if snapshot.data.available && !snapshot.data.truncated {
            findings.push(Finding {
                code: "duplicate_bindings_none".into(),
                severity: Severity::Info,
                title: if zh {
                    "没有重复按键"
                } else {
                    "No duplicate bindings"
                }
                .into(),
                message: if zh {
                    "未发现多个操作使用同一个已绑定按键。"
                } else {
                    "No bound key is assigned to multiple actions."
                }
                .into(),
                evidence: vec![],
                suggestions: vec![],
            });
        }
    } else {
        let details = duplicates
            .iter()
            .take(30)
            .map(|(key, actions)| {
                format!(
                    "{key}: {}",
                    actions.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            })
            .collect::<Vec<_>>();
        let omitted = duplicates.len().saturating_sub(details.len());
        findings.push(Finding {
            code: "duplicate_key_bindings".into(),
            severity: Severity::Warning,
            title: if zh {
                "存在重复按键"
            } else {
                "Duplicate key bindings"
            }
            .into(),
            message: if zh {
                format!(
                    "{} 个按键分配给多个操作：{}{}",
                    duplicates.len(),
                    details.join("；"),
                    if omitted > 0 {
                        format!("；另有 {omitted} 项见明细表。")
                    } else {
                        String::new()
                    }
                )
            } else {
                format!(
                    "{} keys are assigned to multiple actions: {}{}",
                    duplicates.len(),
                    details.join("; "),
                    if omitted > 0 {
                        format!("; {omitted} more are in the table.")
                    } else {
                        String::new()
                    }
                )
            },
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if snapshot.data.truncated {
        findings.push(Finding {
            code: "options_snapshot_truncated".into(),
            severity: Severity::Warning,
            title: if zh {
                "选项列表已截断"
            } else {
                "Options list truncated"
            }
            .into(),
            message: if zh {
                "列表未完整读取，部分按键尚未检查。"
            } else {
                "The list is incomplete; some bindings were not checked."
            }
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if let Some(reason) = &snapshot.data.unavailable {
        findings.push(Finding {
            code: "options_snapshot_unavailable".into(),
            severity: Severity::Warning,
            title: if zh {
                "选项不可用"
            } else {
                "Options unavailable"
            }
            .into(),
            message: reason.chars().take(240).collect(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if !snapshot.data.available && snapshot.data.unavailable.is_none() {
        findings.push(Finding {
            code: "options_snapshot_unavailable".into(),
            severity: Severity::Warning,
            title: if zh {
                "选项不可用"
            } else {
                "Options unavailable"
            }
            .into(),
            message: if zh {
                "无法读取游戏选项。"
            } else {
                "Could not read game options."
            }
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if unknown_key_values > 0 {
        findings.push(Finding { code: "key_values_unrecognized".into(), severity: Severity::Info,
            title: if zh { "部分按键值未识别" } else { "Some key values were not recognized" }.into(),
            message: if zh { format!("{} 个按键值未按标准键盘或鼠标格式参与重复检查。", unknown_key_values) } else { format!("{unknown_key_values} key values did not match the standard keyboard or mouse format and were excluded from duplicate checks.") }, evidence: vec![], suggestions: vec![] });
    }
    if !group_counts.is_empty() {
        let summary = group_counts
            .iter()
            .map(|(name, count)| format!("{name}: {count}"))
            .collect::<Vec<_>>()
            .join(if zh { "、" } else { "; " });
        findings.push(Finding {
            code: "option_group_summary".into(),
            severity: Severity::Info,
            title: if zh {
                "选项分组"
            } else {
                "Options by group"
            }
            .into(),
            message: summary,
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if option_rows.len() < snapshot.data.items.len() {
        findings.push(Finding {
            code: "options_rows_limited".into(),
            severity: Severity::Info,
            title: if zh {
                "选项明细已限制"
            } else {
                "Options details limited"
            }
            .into(),
            message: if zh {
                format!(
                    "显示 {} 项，共 {} 项。",
                    option_rows.len(),
                    snapshot.data.items.len()
                )
            } else {
                format!(
                    "Showing {} of {} options.",
                    option_rows.len(),
                    snapshot.data.items.len()
                )
            },
            evidence: vec![],
            suggestions: vec![],
        });
    }
    let summary = Table {
        columns: if zh {
            vec!["统计".into(), "结果".into()]
        } else {
            vec!["Measure".into(), "Value".into()]
        },
        rows: vec![
            vec![
                if zh { "游戏" } else { "Game" }.into(),
                snapshot.game.name.clone(),
            ],
            vec![
                if zh { "版本" } else { "Version" }.into(),
                snapshot.game.version.clone(),
            ],
            vec![
                if zh { "选项" } else { "Options" }.into(),
                snapshot.data.items.len().to_string(),
            ],
            vec![
                if zh { "按键绑定" } else { "Key bindings" }.into(),
                snapshot
                    .data
                    .items
                    .iter()
                    .filter(|item| item.name.starts_with("key_"))
                    .count()
                    .to_string(),
            ],
            vec![
                if zh {
                    "未绑定按键"
                } else {
                    "Unbound keys"
                }
                .into(),
                unbound.to_string(),
            ],
            vec![
                if zh { "重复按键" } else { "Duplicate keys" }.into(),
                duplicates.len().to_string(),
            ],
        ],
    };
    let groups = Table {
        columns: if zh {
            vec!["分组".into(), "选项数".into()]
        } else {
            vec!["Group".into(), "Options".into()]
        },
        rows: group_counts
            .into_iter()
            .map(|(category, count)| vec![category, count.to_string()])
            .collect(),
    };
    let duplicate_rows = Table {
        columns: if zh {
            vec!["按键".into(), "操作".into()]
        } else {
            vec!["Key".into(), "Actions".into()]
        },
        rows: duplicates
            .into_iter()
            .map(|(key, actions)| vec![key, actions.into_iter().collect::<Vec<_>>().join(", ")])
            .collect(),
    };
    let option_list = Table {
        columns: if zh {
            vec!["分组".into(), "选项".into(), "值".into()]
        } else {
            vec!["Group".into(), "Option".into(), "Value".into()]
        },
        rows: option_rows,
    };
    Report {
        schema_version: 1,
        findings,
        tables: vec![summary, groups, duplicate_rows, option_list],
    }
}

fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    let snapshot = parse_snapshot(bytes)?;
    Ok(analyze_snapshot(snapshot, locale.starts_with("zh")))
}

fn read_input(input: &Input) -> Result<Vec<u8>, Failure> {
    if resources::cancelled() {
        return Err(Failure::Cancelled);
    }
    let metadata = input.describe()?;
    if metadata.media_type != "application/json" {
        return Err(Failure::Unsupported);
    }
    if metadata.byte_length > MAX_INPUT_BYTES as u64 {
        return Err(Failure::Limit);
    }
    let mut bytes = Vec::with_capacity(metadata.byte_length as usize);
    while (bytes.len() as u64) < metadata.byte_length {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let maximum = (metadata.byte_length - bytes.len() as u64).min(65_536) as u32;
        let chunk = input.read_at(bytes.len() as u64, maximum)?;
        if chunk.is_empty() || chunk.len() > maximum as usize {
            return Err(Failure::Io);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

struct ControlsAuditor;
impl exports::launcher::resource_report::reports::Guest for ControlsAuditor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let bytes = read_input(input)?;
        let context = launcher::report_context::context::get();
        let report = analyze_bytes(&bytes, &context.locale)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(ControlsAuditor);

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(items: serde_json::Value, truncated: bool) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":1,"scope":"options",
            "game":{"id":"game-1","name":"Creative","version":"1.21.1"},
            "data":{"items":items,"truncated":truncated,"available":true},
            "revision":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        }))
        .unwrap()
    }
    #[test]
    fn flags_duplicate_bound_keys_but_ignores_unbound_actions_and_groups_options() {
        let input = snapshot(
            serde_json::json!([
                {"name":"key_key.jump","value":"key.keyboard.space"},
                {"name":"key_key.forward","value":"key.keyboard.space"},
                {"name":"key_key.drop","value":"key.keyboard.unknown"},
                {"name":"maxFps","value":120},
                {"name":"rawMouseInput","value":true}
            ]),
            false,
        );
        let report = analyze_bytes(&input, "en-US").unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "duplicate_key_bindings")
        );
        assert_eq!(report.tables[0].rows[4][1], "1");
        assert_eq!(
            report.tables[2].rows,
            vec![vec!["space", "key_key.forward, key_key.jump"]]
        );
        assert!(report.tables[1].rows.iter().any(|row| row[0] == "Display"));
        assert!(report.tables[1].rows.iter().any(|row| row[0] == "Mouse"));
    }
    #[test]
    fn empty_snapshot_is_valid_and_truncation_is_disclosed() {
        let report = analyze_bytes(&snapshot(serde_json::json!([]), true), "zh-CN").unwrap();
        assert_eq!(report.tables[0].rows[2][1], "0");
        assert!(
            report
                .findings
                .iter()
                .all(|finding| finding.code != "duplicate_bindings_none")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "options_snapshot_truncated")
        );
    }
    #[test]
    fn rejects_unknown_schema_and_reports_duplicate_option_names() {
        let mut invalid: serde_json::Value =
            serde_json::from_slice(&snapshot(serde_json::json!([]), false)).unwrap();
        invalid["schemaVersion"] = 2.into();
        assert!(matches!(
            analyze_bytes(&serde_json::to_vec(&invalid).unwrap(), "en"),
            Err(Failure::Unsupported)
        ));
        let duplicate = snapshot(
            serde_json::json!([
                {"name":"maxFps","value":60}, {"name":"maxFps","value":120}
            ]),
            false,
        );
        let report = analyze_bytes(&duplicate, "en").unwrap();
        assert!(report.findings.iter().any(|finding| {
            finding.code == "repeated_option_names" && finding.message.contains("maxFps (2)")
        }));
    }
}

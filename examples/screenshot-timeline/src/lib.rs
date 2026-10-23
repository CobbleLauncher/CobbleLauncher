// SPDX-License-Identifier: MIT
//! Read-only report over a host-scoped screenshot metadata snapshot.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});

use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde::Deserialize;
use std::collections::BTreeMap;

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_SCREENSHOTS: usize = 10_000;
const MAX_ROWS: usize = 1_000;
const MAX_TIMESTAMP: u64 = 253_402_300_799;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Snapshot {
    schema_version: u32,
    scope: String,
    game: Game,
    data: ScreenshotData,
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
struct ScreenshotData {
    items: Vec<Screenshot>,
    truncated: bool,
    available: bool,
    #[serde(default)]
    inspected_at: Option<u64>,
    #[serde(default)]
    unavailable: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Screenshot {
    id: String,
    name: String,
    size_bytes: u64,
    modified_at: Option<u64>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    metadata_error: Option<String>,
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_filename(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && !value.contains('/')
        && !value.contains('\\')
        && !value.chars().any(|character| character.is_control())
        && value != "."
        && value != ".."
}

fn utc_day(timestamp: u64) -> Option<String> {
    if timestamp > MAX_TIMESTAMP {
        return None;
    }
    let days = i64::try_from(timestamp / 86_400).ok()?;
    let z = days.checked_add(719_468)?;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn parse_snapshot(bytes: &[u8]) -> Result<Snapshot, Failure> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Failure::Limit);
    }
    let snapshot: Snapshot = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if snapshot.schema_version != 1
        || snapshot.scope != "screenshots"
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
        || snapshot.data.items.len() > MAX_SCREENSHOTS
        || (!snapshot.data.available && !snapshot.data.items.is_empty())
    {
        return Err(Failure::Unsupported);
    }
    for item in &snapshot.data.items {
        if item.id.len() > 255 || !valid_filename(&item.name) {
            return Err(Failure::Unsupported);
        }
        if item.width.is_some_and(|width| width == 0 || width > 8192)
            || item
                .height
                .is_some_and(|height| height == 0 || height > 8192)
            || item.width.zip(item.height).is_some_and(|(width, height)| {
                u64::from(width) * u64::from(height) > 32 * 1024 * 1024
            })
        {
            return Err(Failure::Unsupported);
        }
    }
    Ok(snapshot)
}

fn analyze_snapshot(snapshot: Snapshot, zh: bool) -> Report {
    let count = snapshot.data.items.len();
    let mut days = BTreeMap::<String, usize>::new();
    let mut unknown_dates = 0usize;
    let mut invalid_dates = 0usize;
    let mut dimension_count = 0usize;
    let mut invalid_metadata = 0usize;
    let mut bytes = 0u64;
    let mut screenshot_rows = Vec::new();
    for item in &snapshot.data.items {
        bytes = bytes.saturating_add(item.size_bytes);
        let date = item.modified_at.and_then(utc_day);
        match (item.modified_at, date.as_ref()) {
            (_, Some(day)) => *days.entry(day.clone()).or_default() += 1,
            (None, _) => unknown_dates += 1,
            (Some(_), None) => invalid_dates += 1,
        }
        let mut item_metadata_invalid = item
            .metadata_error
            .as_ref()
            .is_some_and(|error| !error.trim().is_empty());
        let dimensions = match (item.width, item.height) {
            (Some(width), Some(height)) => {
                dimension_count += 1;
                format!("{width} × {height}")
            }
            (Some(_), None) | (None, Some(_)) => {
                item_metadata_invalid = true;
                if zh { "不完整" } else { "Incomplete" }.into()
            }
            _ => if zh { "未知" } else { "Unknown" }.into(),
        };
        if item_metadata_invalid {
            invalid_metadata += 1;
        }
        if screenshot_rows.len() < MAX_ROWS {
            screenshot_rows.push(vec![
                item.name.clone(),
                date.unwrap_or_else(|| if zh { "未知" } else { "Unknown" }.into()),
                item.size_bytes.to_string(),
                dimensions,
            ]);
        }
    }
    let mut findings = Vec::new();
    if count == 0 && snapshot.data.available {
        findings.push(Finding {
            code: "empty_screenshot_timeline".into(),
            severity: Severity::Info,
            title: if zh { "没有截图" } else { "No screenshots" }.into(),
            message: if zh {
                "此游戏的截图快照为空。"
            } else {
                "The screenshot snapshot for this game is empty."
            }
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if !days.is_empty() {
        let recent = days
            .iter()
            .rev()
            .take(40)
            .map(|(day, count)| format!("{day}: {count}"))
            .collect::<Vec<_>>();
        let omitted = days.len().saturating_sub(recent.len());
        let suffix = if omitted > 0 {
            if zh {
                format!("；另有 {omitted} 天见明细表。")
            } else {
                format!("; {omitted} earlier days are in the table.")
            }
        } else {
            String::new()
        };
        findings.push(Finding {
            code: "screenshot_day_counts".into(),
            severity: Severity::Info,
            title: if zh {
                "每日截图数"
            } else {
                "Screenshots by day"
            }
            .into(),
            message: format!("{}{suffix}", recent.join(if zh { "、" } else { "; " })),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if let Some(reason) = &snapshot.data.unavailable {
        findings.push(Finding {
            code: "screenshot_metadata_unavailable".into(),
            severity: Severity::Warning,
            title: if zh {
                "截图元数据不可用"
            } else {
                "Screenshot metadata unavailable"
            }
            .into(),
            message: reason.chars().take(240).collect(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if !snapshot.data.available && snapshot.data.unavailable.is_none() {
        findings.push(Finding {
            code: "screenshot_metadata_unavailable".into(),
            severity: Severity::Warning,
            title: if zh {
                "截图元数据不可用"
            } else {
                "Screenshot metadata unavailable"
            }
            .into(),
            message: if zh {
                "宿主未能读取截图元数据。"
            } else {
                "The host could not read screenshot metadata."
            }
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if snapshot.data.truncated {
        findings.push(Finding {
            code: "screenshot_list_truncated".into(),
            severity: Severity::Warning,
            title: if zh {
                "列表已截断"
            } else {
                "List truncated"
            }
            .into(),
            message: if zh {
                "日期计数仅包含快照中的项目。"
            } else {
                "Day counts include only items in this snapshot."
            }
            .into(),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if unknown_dates + invalid_dates > 0 {
        findings.push(Finding { code: "screenshot_dates_incomplete".into(), severity: Severity::Info,
            title: if zh { "日期不完整" } else { "Incomplete dates" }.into(),
            message: if zh { format!("{} 张截图没有日期，{} 个时间戳无效。", unknown_dates, invalid_dates) } else { format!("{unknown_dates} screenshots have no date; {invalid_dates} timestamps are invalid.") }, evidence: vec![], suggestions: vec![] });
    }
    if invalid_metadata > 0 {
        findings.push(Finding {
            code: "screenshot_image_metadata_invalid".into(),
            severity: Severity::Warning,
            title: if zh {
                "图片元数据不完整"
            } else {
                "Image metadata incomplete"
            }
            .into(),
            message: if zh {
                format!("{} 项已知图片元数据无效或不完整。", invalid_metadata)
            } else {
                format!(
                    "{invalid_metadata} known image metadata entries are invalid or incomplete."
                )
            },
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if screenshot_rows.len() < count {
        findings.push(Finding {
            code: "screenshot_rows_limited".into(),
            severity: Severity::Info,
            title: if zh {
                "截图明细已限制"
            } else {
                "Screenshot details limited"
            }
            .into(),
            message: if zh {
                format!("显示 {} 项，共 {count} 项。", screenshot_rows.len())
            } else {
                format!("Showing {} of {count} screenshots.", screenshot_rows.len())
            },
            evidence: vec![],
            suggestions: vec![],
        });
    }
    let totals = Table {
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
                if zh { "截图" } else { "Screenshots" }.into(),
                count.to_string(),
            ],
            vec![
                if zh {
                    "有日期的天数"
                } else {
                    "Days with screenshots"
                }
                .into(),
                days.len().to_string(),
            ],
            vec![
                if zh {
                    "已知尺寸"
                } else {
                    "Known dimensions"
                }
                .into(),
                dimension_count.to_string(),
            ],
            vec![
                if zh {
                    "总大小（字节）"
                } else {
                    "Total size (bytes)"
                }
                .into(),
                bytes.to_string(),
            ],
        ],
    };
    let days_table = Table {
        columns: if zh {
            vec!["日期（UTC）".into(), "截图".into()]
        } else {
            vec!["Date (UTC)".into(), "Screenshots".into()]
        },
        rows: days
            .into_iter()
            .rev()
            .map(|(day, count)| vec![day, count.to_string()])
            .collect(),
    };
    let screenshot_table = Table {
        columns: if zh {
            vec![
                "文件名".into(),
                "日期（UTC）".into(),
                "大小（字节）".into(),
                "尺寸".into(),
            ]
        } else {
            vec![
                "Filename".into(),
                "Date (UTC)".into(),
                "Size (bytes)".into(),
                "Dimensions".into(),
            ]
        },
        rows: screenshot_rows,
    };
    let _ = snapshot.data.inspected_at;
    Report {
        schema_version: 1,
        findings,
        tables: vec![totals, days_table, screenshot_table],
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

struct ScreenshotTimeline;
impl exports::launcher::resource_report::reports::Guest for ScreenshotTimeline {
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
export!(ScreenshotTimeline);

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(data: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":1,"scope":"screenshots",
            "game":{"id":"game-1","name":"Creative","version":"1.21.1"},
            "data":data,"revision":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        })).unwrap()
    }
    #[test]
    fn groups_screenshots_by_utc_day_and_keeps_unknown_metadata_explicit() {
        let input = snapshot(serde_json::json!({"items":[
            {"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","name":"first.png","sizeBytes":42,"modifiedAt":0},
            {"id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","name":"second.png","sizeBytes":58,"modifiedAt":1,"width":16,"height":8}
        ],"truncated":false,"available":true}));
        let report = analyze_bytes(&input, "en-US").unwrap();
        assert_eq!(report.tables[0].rows[2][1], "2");
        assert_eq!(report.tables[0].rows[3][1], "1");
        assert_eq!(report.tables[1].rows, vec![vec!["1970-01-01", "2"]]);
        assert_eq!(report.tables[2].rows[0][3], "Unknown");
        assert_eq!(report.tables[2].rows[1][3], "16 × 8");
    }
    #[test]
    fn reports_empty_and_truncated_inventory_and_rejects_wrong_scope_or_version() {
        let input = snapshot(serde_json::json!({"items":[],"truncated":true,"available":true}));
        let report = analyze_bytes(&input, "zh-CN").unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "empty_screenshot_timeline")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "screenshot_list_truncated")
        );
        let mut invalid: serde_json::Value = serde_json::from_slice(&input).unwrap();
        invalid["schemaVersion"] = 2.into();
        assert!(matches!(
            analyze_bytes(&serde_json::to_vec(&invalid).unwrap(), "en"),
            Err(Failure::Unsupported)
        ));
    }
    #[test]
    fn rejects_invalid_known_image_dimensions_and_filename_paths() {
        for item in [
            serde_json::json!({"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","name":"bad.png","sizeBytes":0,"modifiedAt":null,"width":0,"height":2}),
            serde_json::json!({"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","name":"folder/bad.png","sizeBytes":0,"modifiedAt":null}),
        ] {
            let input =
                snapshot(serde_json::json!({"items":[item],"truncated":false,"available":true}));
            assert!(matches!(
                analyze_bytes(&input, "en"),
                Err(Failure::Unsupported)
            ));
        }
    }
}

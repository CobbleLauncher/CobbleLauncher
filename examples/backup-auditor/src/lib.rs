// SPDX-License-Identifier: MIT
//! Original read-only audit of retained replacement-backup metadata.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
use serde_json::Value;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_ROWS: usize = 128;
fn tr<'a>(zh: bool, en: &'a str, cn: &'a str) -> &'a str {
    if zh { cn } else { en }
}
fn text(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("—")
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect()
}
fn note(report: &mut Report, code: &str, severity: Severity, message: String, zh: bool) {
    if report.findings.len() < 32 {
        report.findings.push(Finding {
            code: code.into(),
            severity,
            title: tr(zh, "Backup review", "备份检查").into(),
            message,
            evidence: vec![],
            suggestions: vec![],
        });
    }
}
fn analyze_bytes(bytes: &[u8], locale: &str) -> Result<Report, Failure> {
    if bytes.len() > MAX_BYTES {
        return Err(Failure::Limit);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if root["schemaVersion"] != 1
        || root["scope"] != "backups"
        || !root["game"].is_object()
        || !root["revision"].is_string()
    {
        return Err(Failure::Unsupported);
    }
    let zh = locale == "zh-CN";
    let mut report = Report {
        schema_version: 1,
        findings: vec![],
        tables: vec![Table {
            columns: vec![
                tr(zh, "World", "世界").into(),
                tr(zh, "Retained replacements", "保留的替换备份").into(),
                tr(zh, "Latest backup age", "最近备份时间").into(),
            ],
            rows: vec![],
        }],
    };
    note(&mut report,"encrypted_snapshots_not_inspected",Severity::Info,tr(zh,"Encrypted snapshot storage was not opened. These records do not verify backup contents or a successful restore.","未打开加密快照存储；这些记录不代表已验证备份内容或恢复成功。").into(),zh);
    let Some(items) = root["data"]["items"]
        .as_array()
        .filter(|_| root["data"]["available"] == true && root["data"]["unavailable"].is_null())
    else {
        note(&mut report,"backup_index_unavailable",Severity::Warning,tr(zh,"The replacement-backup index is unavailable. Backup coverage cannot be determined.","替换备份索引不可用，无法确认备份覆盖情况。").into(),zh);
        return Ok(report);
    };
    let Some(worlds) = root["data"]["worlds"].as_array() else {
        note(&mut report,"backup_world_inventory_unavailable",Severity::Warning,tr(zh,"Current world inventory is unavailable. Worlds without retained replacements cannot be identified.","当前世界清单不可用，无法识别没有保留替换备份的世界。").into(),zh);
        return Ok(report);
    };
    let now = root["data"]["inspectedAt"].as_u64();
    let mut no_backup = 0;
    let mut older = 0;
    let mut unknown_age = 0;
    for world in worlds.iter().take(MAX_ROWS) {
        let Some(world_id) = world["id"].as_str() else {
            continue;
        };
        let matched: Vec<_> = items
            .iter()
            .filter(|item| {
                item["worldId"].as_str() == Some(world_id) && item["kind"] == "replacement"
            })
            .collect();
        let latest = matched
            .iter()
            .filter_map(|item| item["createdAt"].as_u64())
            .max();
        let age = if matched.is_empty() {
            no_backup += 1;
            tr(zh, "No retained replacement", "没有保留的替换备份").into()
        } else {
            match (now, latest) {
                (Some(now), Some(at)) if at > 0 && at <= now => {
                    let days = (now - at) / 86400;
                    if days >= 30 {
                        older += 1
                    }
                    if days == 0 {
                        tr(zh, "Within one day", "一天内").into()
                    } else {
                        format!(
                            "{days} {}",
                            tr(zh, if days == 1 { "day ago" } else { "days ago" }, "天前")
                        )
                    }
                }
                _ => {
                    unknown_age += 1;
                    tr(zh, "Unknown", "未知").into()
                }
            }
        };
        report.tables[0]
            .rows
            .push(vec![text(&world["name"]), matched.len().to_string(), age]);
    }
    if no_backup > 0 {
        note(
            &mut report,
            "worlds_without_replacement",
            Severity::Warning,
            format!(
                "{no_backup} {}",
                tr(
                    zh,
                    "worlds have no retained replacement backup in this index. Consider creating a world snapshot or portable export.",
                    "个世界在此索引中没有保留的替换备份，可考虑创建世界快照或导出。"
                )
            ),
            zh,
        );
    }
    if older > 0 {
        note(
            &mut report,
            "replacement_older_than_30_days",
            Severity::Warning,
            format!(
                "{older} {}",
                tr(
                    zh,
                    "worlds have a latest retained replacement at least 30 days old. Review whether a newer backup is needed.",
                    "个世界最近保留的替换备份已超过 30 天，请确认是否需要更新备份。"
                )
            ),
            zh,
        );
    }
    if unknown_age > 0 {
        note(
            &mut report,
            "replacement_age_unknown",
            Severity::Warning,
            format!(
                "{unknown_age} {}",
                tr(
                    zh,
                    "worlds have backup records with an unknown or future creation time.",
                    "个世界的备份时间未知或在未来。"
                )
            ),
            zh,
        );
    }
    let listed = report.tables[0].rows.len();
    note(
        &mut report,
        "backup_inventory_summary",
        Severity::Info,
        format!(
            "{}: {listed}; {}: {}",
            tr(zh, "Worlds reviewed", "已检查世界"),
            tr(zh, "Replacement records", "替换备份记录"),
            items.len()
        ),
        zh,
    );
    if root["data"]["truncated"] == true || worlds.len() > MAX_ROWS {
        note(
            &mut report,
            "backup_inventory_partial",
            Severity::Warning,
            tr(
                zh,
                "Only part of the inventory is shown; coverage counts describe listed worlds only.",
                "仅显示部分清单，覆盖数量仅适用于已列出的世界。",
            )
            .into(),
            zh,
        );
    }
    Ok(report)
}
struct BackupAuditor;
impl exports::launcher::resource_report::reports::Guest for BackupAuditor {
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
export!(BackupAuditor);
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn snapshot() -> Value {
        json!({"schemaVersion":1,"scope":"backups","game":{"id":"g","name":"G","version":"1.21.1"},"revision":"r","data":{"available":true,"inspectedAt":4000000,"encryptedSnapshots":"not_inspected","worlds":[{"id":"a","name":"First"},{"id":"b","name":"Second"}],"items":[{"id":"old","worldId":"a","name":"First","kind":"replacement","createdAt":1}]}})
    }
    #[test]
    fn finds_missing_and_old_replacements_without_restore_claims() {
        let report = analyze_bytes(&serde_json::to_vec(&snapshot()).unwrap(), "en").unwrap();
        assert_eq!(report.tables[0].rows.len(), 2);
        assert_eq!(report.tables[0].rows[1][1], "0");
        for code in [
            "worlds_without_replacement",
            "replacement_older_than_30_days",
            "encrypted_snapshots_not_inspected",
        ] {
            assert!(report.findings.iter().any(|f| f.code == code));
        }
        assert!(report.findings[0].message.contains("do not verify"));
    }
    #[test]
    fn unavailable_index_does_not_report_missing_backups() {
        let mut root = snapshot();
        root["data"]["available"] = json!(false);
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "zh-CN").unwrap();
        assert!(report.tables[0].rows.is_empty());
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "backup_index_unavailable")
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.code == "worlds_without_replacement")
        );
    }
    #[test]
    fn latest_record_wins_and_future_time_is_unknown() {
        let mut root = snapshot();
        root["data"]["items"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":"new","worldId":"a","kind":"replacement","createdAt":3999999}));
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "en").unwrap();
        assert_eq!(report.tables[0].rows[0][2], "Within one day");
        assert!(
            !report
                .findings
                .iter()
                .any(|f| f.code == "replacement_older_than_30_days")
        );
        root["data"]["items"][1]["createdAt"] = json!(u64::MAX);
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "en").unwrap();
        assert_eq!(report.tables[0].rows[0][2], "Unknown");
    }
    #[test]
    fn invalid_scope_and_limits_are_explicit() {
        assert!(matches!(
            analyze_bytes(b"{}", "en"),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            analyze_bytes(&vec![0; MAX_BYTES + 1], "en"),
            Err(Failure::Limit)
        ));
        let mut root = snapshot();
        root["data"]["worlds"] = json!(
            (0..200)
                .map(|n| json!({"id":n.to_string(),"name":"中".repeat(400)}))
                .collect::<Vec<_>>()
        );
        let report = analyze_bytes(&serde_json::to_vec(&root).unwrap(), "zh-CN").unwrap();
        assert_eq!(report.tables[0].rows.len(), 128);
        assert_eq!(report.tables[0].rows[0][0].chars().count(), 160);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "backup_inventory_partial")
        );
    }
}

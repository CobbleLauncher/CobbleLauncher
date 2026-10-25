// SPDX-License-Identifier: MIT
//! Reports facts about declared IDs. Version satisfiability is deliberately unknown.
use super::{Failure, Finding, Report, Severity, Table};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const RUNTIME_IDS: &[&str] = &[
    "minecraft",
    "java",
    "fabricloader",
    "quilt_loader",
    "forge",
    "neoforge",
];
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Failure> {
    value[key]
        .as_str()
        .filter(|text| !text.is_empty() && text.len() <= 1024)
        .ok_or(Failure::Unsupported)
}
pub(super) fn analyze(root: Value, zh: bool) -> Result<Report, Failure> {
    if root["schemaVersion"] != 1 || root["scope"] != "mods" {
        return Err(Failure::Unsupported);
    }
    let mods = root["data"]["items"]
        .as_array()
        .ok_or(Failure::Unsupported)?;
    if mods.len() > 512 {
        return Err(Failure::Limit);
    }
    let mut providers: BTreeMap<&str, Vec<bool>> = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut disabled = 0usize;
    let mut unknown = 0usize;
    for item in mods {
        if !names.insert(text(item, "id")?) {
            return Err(Failure::Unsupported);
        }
        let off = item["disabled"].as_bool().ok_or(Failure::Unsupported)?;
        disabled += usize::from(off);
        let ids = item["declaredIds"].as_array().ok_or(Failure::Unsupported)?;
        if ids.len() > 65 {
            return Err(Failure::Limit);
        }
        unknown += usize::from(ids.is_empty());
        let mut unique = BTreeSet::new();
        for id in ids {
            let id = id
                .as_str()
                .filter(|id| id.len() <= 1024)
                .ok_or(Failure::Unsupported)?;
            if unique.insert(id) {
                providers.entry(id).or_default().push(off);
            }
        }
    }
    let mut required = 0usize;
    let mut absent = 0usize;
    let mut disabled_provider = 0usize;
    let mut runtime = 0usize;
    let mut ambiguous = 0usize;
    for item in mods {
        let dependencies = item["dependencies"]
            .as_array()
            .ok_or(Failure::Unsupported)?;
        if dependencies.len() > 64 {
            return Err(Failure::Limit);
        }
        for dep in dependencies {
            let id = text(dep, "id")?;
            let optional = dep["optional"].as_bool().ok_or(Failure::Unsupported)?;
            required += usize::from(!optional);
            let candidates = providers.get(id);
            let enabled = candidates.map_or(0, |items| items.iter().filter(|off| !**off).count());
            if enabled > 1 {
                ambiguous += 1;
            } else if enabled == 0 {
                if candidates.is_some() {
                    disabled_provider += usize::from(!optional);
                } else if RUNTIME_IDS.contains(&id) {
                    runtime += 1;
                } else {
                    absent += usize::from(!optional);
                }
            }
        }
    }
    Ok(report(
        zh,
        [
            mods.len(),
            disabled,
            unknown,
            required,
            absent,
            disabled_provider,
            runtime,
            ambiguous,
        ],
    ))
}
fn report(zh: bool, counts: [usize; 8]) -> Report {
    let labels = if zh {
        [
            "模组",
            "已禁用",
            "无可读取声明",
            "必需声明",
            "无已声明匹配",
            "仅有已禁用提供者",
            "运行环境未检查",
            "多个提供者",
        ]
    } else {
        [
            "Mods",
            "Disabled",
            "No readable declarations",
            "Required declarations",
            "No declared match",
            "Disabled providers only",
            "Runtime unchecked",
            "Multiple providers",
        ]
    };
    Report {
        schema_version: 1,
        findings: vec![Finding {
            code: "declared_dependency_inventory".into(),
            severity: if counts[4] + counts[5] > 0 { Severity::Warning } else { Severity::Info },
            title: if zh { "声明依赖" } else { "Declared dependencies" }.into(),
            message: if zh {
                "按声明 ID 匹配已安装模组。缺少声明或未支持的元数据会影响匹配；未检查版本范围及游戏兼容性。"
            } else {
                "Installed Mods are matched by declared ID. Missing declarations or unsupported metadata affect matching; version ranges and game compatibility are unchecked."
            }.into(),
            evidence: vec![],
            suggestions: vec![],
        }],
        tables: vec![Table {
            columns: if zh { vec!["字段".into(), "值".into()] } else { vec!["Field".into(), "Value".into()] },
            rows: labels.into_iter().zip(counts).map(|(label, count)| vec![label.into(),count.to_string()]).collect(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn localized_facts_never_claim_version_satisfiability() {
        let value = report(false, [8, 1, 2, 3, 1, 0, 2, 1]);
        assert_eq!(value.tables[0].rows[4], ["No declared match", "1"]);
        assert!(value.findings[0].message.contains("unchecked"));
        assert_eq!(report(true, [0; 8]).tables[0].columns, ["字段", "值"]);
    }
    #[test]
    fn wrong_scope_and_over_budget_are_explicit() {
        assert!(matches!(
            analyze(
                serde_json::json!({"schemaVersion":1,"scope":"worlds"}),
                false
            ),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(
            analyze(
                serde_json::json!({"schemaVersion":1,"scope":"mods","data":{"items":vec![Value::Null;513]}}),
                false
            ),
            Err(Failure::Limit)
        ));
    }
    #[test]
    fn counts_disabled_runtime_absent_and_aliases_without_resolving_ranges() {
        let item = |id: &str, ids: Vec<&str>, disabled: bool, dependencies: Value| serde_json::json!({"id":id,"declaredIds":ids,"disabled":disabled,"dependencies":dependencies});
        let root = serde_json::json!({"schemaVersion":1,"scope":"mods","data":{"items":[
            item("main.jar",vec!["main"],false,serde_json::json!([
                {"id":"alias","version":">=99","optional":false},
                {"id":"off","version":"*","optional":false},
                {"id":"missing","version":null,"optional":false},
                {"id":"java","version":">=21","optional":false}
            ])),
            item("helper.jar",vec!["helper","alias"],false,serde_json::json!([])),
            item("off.jar",vec!["off"],true,serde_json::json!([]))
        ]}});
        let value = analyze(root, false).unwrap();
        assert_eq!(value.tables[0].rows[4][1], "1");
        assert_eq!(value.tables[0].rows[5][1], "1");
        assert_eq!(value.tables[0].rows[6][1], "1");
    }
}

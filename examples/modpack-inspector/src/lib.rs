// SPDX-License-Identifier: MIT
//! Read-only reports over raw indices and host-prepared MRPACK metadata snapshots.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
use exports::launcher::resource_report::reports::{Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
const SNAPSHOT_MEDIA: &str = "application/vnd.cobble.mrpack-inspection+json";
const MAX_SNAPSHOT_BYTES: usize = 8 * 1024 * 1024;

fn analyze_snapshot(bytes: &[u8]) -> Result<Report, Failure> {
    use cobble_modpack_inspector::{InspectionRequest, inspect_request};
    if bytes.len() > MAX_SNAPSHOT_BYTES {
        return Err(Failure::Limit);
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    if value.get("protocol").is_some() {
        return analyze_page(value);
    }
    let request: InspectionRequest =
        serde_json::from_slice(bytes).map_err(|_| Failure::Unsupported)?;
    let inspection = inspect_request(&request).map_err(map_inspection_error)?;
    let value = serde_json::to_value(inspection).map_err(|_| Failure::Io)?;
    let mut tables = vec![
        Table {
            columns: vec!["Pack".into(), "Field".into(), "Value".into()],
            rows: vec![],
        },
        Table {
            columns: vec![
                "Pack".into(),
                "Dependency".into(),
                "Declared version".into(),
            ],
            rows: vec![],
        },
        Table {
            columns: vec!["Source".into(), "Path".into(), "Metadata".into()],
            rows: vec![],
        },
        Table {
            columns: vec![
                "Source".into(),
                "Change".into(),
                "Path".into(),
                "Before".into(),
                "After".into(),
            ],
            rows: vec![],
        },
    ];
    let mut budget = ListingBudget::default();
    for pack in ["current", "baseline"] {
        let composition = &value[pack];
        if composition.is_null() {
            continue;
        }
        for field in [
            "name",
            "version_id",
            "file_count",
            "declared_download_bytes",
            "client",
            "server",
        ] {
            budget.add(
                &mut tables[0],
                vec![
                    pack.into(),
                    field.into(),
                    display(&composition["summary"][field]),
                ],
            );
        }
        budget.add(
            &mut tables[0],
            vec![
                pack.into(),
                "override_files".into(),
                composition["overrides"]
                    .as_array()
                    .map_or(0, Vec::len)
                    .to_string(),
            ],
        );
        if let Some(dependencies) = composition["summary"]["dependencies"].as_object() {
            for (key, version) in dependencies {
                budget.add(
                    &mut tables[1],
                    vec![pack.into(), key.clone(), display(version)],
                );
            }
        }
    }
    // Comparison counts precede listings so the complete totals always remain visible.
    if let Some(diff) = value["diff"].as_object() {
        for source in ["declared_files", "overrides"] {
            for change in ["added", "removed", "changed", "unchanged"] {
                let count = diff[source][change].as_array().map_or(0, Vec::len);
                budget.add(
                    &mut tables[0],
                    vec![
                        "comparison".into(),
                        format!("{source} {change}"),
                        count.to_string(),
                    ],
                );
            }
        }
    }
    // Prioritize changed files over current composition for large comparisons.
    for change in ["changed", "added", "removed", "unchanged"] {
        for source in ["declared_files", "overrides"] {
            if let Some(files) = value["diff"][source][change].as_array() {
                for file in files {
                    let (before, after) = match change {
                        "changed" => (display(&file["before"]), display(&file["after"])),
                        "removed" => (display(file), String::new()),
                        "unchanged" => (display(file), display(file)),
                        _ => (String::new(), display(file)),
                    };
                    budget.add(
                        &mut tables[3],
                        vec![
                            source.into(),
                            change.into(),
                            display(&file["path"]),
                            before,
                            after,
                        ],
                    );
                }
            }
        }
    }
    for (field, source) in [("files", "declared"), ("overrides", "override")] {
        if let Some(files) = value["current"][field].as_array() {
            for file in files {
                budget.add(
                    &mut tables[2],
                    vec![source.into(), display(&file["path"]), display(file)],
                );
            }
        }
    }
    let mut findings = vec![Finding {
        code: "pack_inspection".into(),
        severity: Severity::Info,
        title: "Pack contents".into(),
        message:
            "Declared metadata and archive overrides. No compatibility or installability check."
                .into(),
        evidence: vec![],
        suggestions: vec![],
    }];
    if !value["diff"].is_null() {
        findings.push(Finding {
            code: "comparison_basis".into(),
            severity: Severity::Info,
            title: "Comparison".into(),
            message: display(&value["diff"]["comparison_basis"]),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    if budget.omitted > 0 || budget.shortened > 0 {
        findings.push(Finding {
            code: "bounded_listing".into(),
            severity: Severity::Warning,
            title: "Listing limit".into(),
            message: format!(
                "{} rows unshown; {} cells shortened. Comparison totals include every file.",
                budget.omitted, budget.shortened
            ),
            evidence: vec![],
            suggestions: vec![],
        });
    }
    Ok(Report {
        schema_version: 1,
        findings,
        tables,
    })
}

fn analyze_page(value: serde_json::Value) -> Result<Report, Failure> {
    use cobble_modpack_inspector::{InspectionEnvelope, MAX_PAGE_ROWS};
    let envelope: InspectionEnvelope =
        serde_json::from_value(value).map_err(|_| Failure::Unsupported)?;
    envelope
        .validate_protocol()
        .map_err(|_| Failure::Unsupported)?;
    let value = serde_json::to_value(envelope).map_err(|_| Failure::Io)?;
    let mut report = Report {
        schema_version: 1,
        findings: vec![],
        tables: vec![],
    };
    match value["kind"].as_str() {
        Some("page") => {
            let page = &value["page"];
            let rows = page["rows"].as_array().ok_or(Failure::Unsupported)?;
            if rows.len() > MAX_PAGE_ROWS as usize {
                return Err(Failure::Limit);
            }
            let mut table = Table {
                columns: vec![
                    "Source".into(),
                    "Change".into(),
                    "Path".into(),
                    "Before".into(),
                    "After".into(),
                ],
                rows: vec![],
            };
            let mut needs_detail = false;
            for row in rows {
                let mut preview = |field: &str| -> Result<String, Failure> {
                    let cell = &row[field];
                    if cell.is_null() {
                        return Ok(String::new());
                    }
                    let text = cell["text"].as_str().ok_or(Failure::Unsupported)?;
                    if text.len() > 512 {
                        return Err(Failure::Limit);
                    }
                    let complete = cell["complete"].as_bool().ok_or(Failure::Unsupported)?;
                    needs_detail |= !complete;
                    Ok(text.into())
                };
                let path = preview("path")?;
                let before = preview("before")?;
                let after = preview("after")?;
                table.rows.push(vec![
                    display(&row["source"]),
                    row["change"].as_str().unwrap_or_default().into(),
                    path,
                    before,
                    after,
                ]);
            }
            let summary = Table {
                columns: vec!["Field".into(), "Value".into()],
                rows: vec![
                    vec!["Section".into(), display(&page["query"]["section"])],
                    vec!["Total rows".into(), display(&page["total_rows"])],
                    vec!["Matching rows".into(), display(&page["matched_rows"])],
                    vec!["Offset".into(), display(&page["query"]["offset"])],
                    vec!["Page rows".into(), rows.len().to_string()],
                ],
            };
            report.tables = vec![summary, table];
            if needs_detail {
                report.findings.push(Finding {
                    code: "field_details_available".into(),
                    severity: Severity::Info,
                    title: "Full metadata".into(),
                    message:
                        "Some cells are previews. Open a row's details to read the complete value."
                            .into(),
                    evidence: vec![],
                    suggestions: vec![],
                });
            }
        }
        Some("detail") => {
            let detail = &value["detail"];
            let text = detail["text"].as_str().ok_or(Failure::Unsupported)?;
            if text.len() > 2048 {
                return Err(Failure::Limit);
            }
            report.tables.push(Table {
                columns: vec!["Field".into(), "Value".into()],
                rows: vec![
                    vec!["Field".into(), display(&detail["field"])],
                    vec!["Byte offset".into(), display(&detail["offset"])],
                    vec!["Total bytes".into(), display(&detail["total_bytes"])],
                    vec!["Content".into(), text.into()],
                ],
            });
        }
        _ => return Err(Failure::Unsupported),
    }
    Ok(report)
}

fn display(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

#[derive(Default)]
struct ListingBudget {
    rows: usize,
    bytes: usize,
    omitted: usize,
    shortened: usize,
}
impl ListingBudget {
    fn add(&mut self, table: &mut Table, mut row: Vec<String>) {
        for cell in &mut row {
            if cell.len() > 2048 {
                let mut end = 2000;
                while !cell.is_char_boundary(end) {
                    end -= 1;
                }
                cell.truncate(end);
                cell.push_str(" … [shortened]");
                self.shortened += 1;
            }
        }
        // Reserve ample space for JSON escaping, columns and findings within 1 MiB.
        let size = serde_json::to_vec(&row).map_or(usize::MAX, |bytes| bytes.len());
        if self.rows >= 2000 || self.bytes.saturating_add(size) > 400_000 {
            self.omitted += 1;
            return;
        }
        self.rows += 1;
        self.bytes += size;
        table.rows.push(row);
    }
}
fn analyze_bytes(bytes: &[u8]) -> Result<Report, Failure> {
    let index = cobble_modpack_inspector::inspect_index(bytes).map_err(map_inspection_error)?;
    // Fail as a whole rather than silently truncate untrusted report strings/rows.
    if index.name.len() > 2048
        || index.version_id.len() > 2048
        || index.dependencies.len() > 2048 - 7
        || index
            .dependencies
            .iter()
            .any(|(k, v)| k.len() > 2048 || v.len() > 2048)
    {
        return Err(Failure::Limit);
    }
    let summary = Table {
        columns: vec!["Field".into(), "Value".into()],
        rows: vec![
            vec!["Name".into(), index.name],
            vec!["Version".into(), index.version_id],
            vec!["Declared files".into(), index.file_count.to_string()],
            vec![
                "Declared download bytes".into(),
                index.declared_download_bytes.to_string(),
            ],
        ],
    };
    let dependencies = Table {
        columns: vec!["Dependency".into(), "Declared version".into()],
        rows: index
            .dependencies
            .into_iter()
            .map(|(k, v)| vec![k, v])
            .collect(),
    };
    let sides = Table {
        columns: vec![
            "Side".into(),
            "Required".into(),
            "Optional".into(),
            "Unsupported".into(),
        ],
        rows: vec![
            vec![
                "Client".into(),
                index.client.required.to_string(),
                index.client.optional.to_string(),
                index.client.unsupported.to_string(),
            ],
            vec![
                "Server".into(),
                index.server.required.to_string(),
                index.server.optional.to_string(),
                index.server.unsupported.to_string(),
            ],
        ],
    };
    Ok(Report {
        schema_version: 1,
        findings: vec![Finding {
            code: "declared_index_summary".into(),
            severity: Severity::Info,
            title: "Declared pack contents".into(),
            message: "Pack details, dependencies and file totals from modrinth.index.json.".into(),
            evidence: vec![],
            suggestions: vec![],
        }],
        tables: vec![summary, dependencies, sides],
    })
}
fn map_inspection_error(error: cobble_modpack_inspector::InspectError) -> Failure {
    use cobble_modpack_inspector::InspectError;
    match error {
        InspectError::InputTooLarge
        | InspectError::TooManyFiles
        | InspectError::DownloadSizeOverflow => Failure::Limit,
        _ => Failure::Unsupported,
    }
}
fn read_input(input: &Input) -> Result<(String, Vec<u8>), Failure> {
    if resources::cancelled() {
        return Err(Failure::Cancelled);
    }
    let metadata = input.describe()?;
    let limit = match metadata.media_type.as_str() {
        SNAPSHOT_MEDIA => MAX_SNAPSHOT_BYTES,
        "application/json" => cobble_modpack_inspector::MAX_INDEX_BYTES,
        _ => return Err(Failure::Unsupported),
    };
    if metadata.byte_length > limit as u64 {
        return Err(Failure::Limit);
    }
    let mut bytes = Vec::with_capacity(metadata.byte_length as usize);
    while (bytes.len() as u64) < metadata.byte_length {
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        let maximum = (metadata.byte_length - bytes.len() as u64).min(65536) as u32;
        let chunk = input.read_at(bytes.len() as u64, maximum)?;
        if chunk.is_empty() || chunk.len() > maximum as usize {
            return Err(Failure::Io);
        }
        bytes.extend_from_slice(&chunk);
    }
    if resources::cancelled() {
        return Err(Failure::Cancelled);
    }
    Ok((metadata.media_type, bytes))
}
fn apply_presentation(
    mut report: Report,
    locale: &str,
    settings: &serde_json::Value,
    overview: bool,
) -> Report {
    if overview
        && settings
            .get("show_dependencies")
            .and_then(|value| value.as_bool())
            == Some(false)
    {
        report
            .tables
            .retain(|table| !table.columns.iter().any(|column| column == "Dependency"));
    }
    if locale == "zh-CN" {
        for table in &mut report.tables {
            let field_table = table.columns == ["Field", "Value"];
            let side_table = table.columns.first().is_some_and(|column| column == "Side");
            if field_table || side_table {
                for row in &mut table.rows {
                    if let Some(label) = row.first_mut()
                        && let Some(translated) = zh_label(label)
                    {
                        *label = translated.into();
                    }
                }
            }
            for column in &mut table.columns {
                if let Some(translated) = zh_label(column) {
                    *column = translated.into();
                }
            }
        }
        for finding in &mut report.findings {
            if let Some(translated) = zh_label(&finding.title) {
                finding.title = translated.into();
            }
            if finding.code == "declared_index_summary" {
                finding.message = "modrinth.index.json 声明的整合包信息、依赖和文件统计。".into();
            }
        }
    }
    report
}
fn zh_label(value: &str) -> Option<&'static str> {
    Some(match value {
        "Field" => "字段",
        "Value" => "值",
        "Name" => "名称",
        "Version" => "版本",
        "Declared files" => "声明文件数",
        "Declared download bytes" => "声明下载字节数",
        "Dependency" => "依赖",
        "Declared version" => "声明版本",
        "Side" => "运行端",
        "Required" => "必需",
        "Optional" => "可选",
        "Unsupported" => "不支持",
        "Client" => "客户端",
        "Server" => "服务端",
        "Pack" => "整合包",
        "Source" => "来源",
        "Path" => "路径",
        "Metadata" => "元数据",
        "Change" => "变化",
        "Before" => "之前",
        "After" => "之后",
        "Section" => "分类",
        "Total rows" => "总行数",
        "Matching rows" => "匹配行数",
        "Offset" => "偏移量",
        "Page rows" => "本页行数",
        "Byte offset" => "字节偏移量",
        "Total bytes" => "总字节数",
        "Content" => "内容",
        "Declared pack contents" => "整合包声明内容",
        "Pack contents" => "整合包内容",
        "Comparison" => "比较",
        "Listing limit" => "列表限制",
        "Full metadata" => "完整元数据",
        _ => return None,
    })
}
struct ModpackInspector;
impl exports::launcher::resource_report::reports::Guest for ModpackInspector {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let (media_type, bytes) = read_input(input)?;
        let report = if media_type == SNAPSHOT_MEDIA {
            analyze_snapshot(&bytes)?
        } else {
            analyze_bytes(&bytes)?
        };
        let context = launcher::report_context::context::get();
        let settings: serde_json::Value =
            serde_json::from_str(&context.settings_json).map_err(|_| Failure::Unsupported)?;
        let overview = media_type != "application/vnd.cobble.mrpack-inspection+json"
            || serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .is_some_and(|value| value.get("protocol").is_none());
        let report = apply_presentation(report, &context.locale, &settings, overview);
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(ModpackInspector);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_changes_localized_headers_and_overview_dependencies_only() {
        let original = analyze_bytes(include_bytes!("../fixtures/modrinth.index.json")).unwrap();
        let zh = apply_presentation(
            original,
            "zh-CN",
            &serde_json::json!({"show_dependencies":false}),
            true,
        );
        assert_eq!(zh.tables.len(), 2);
        assert_eq!(zh.tables[0].columns, ["字段", "值"]);
        assert_eq!(zh.tables[0].rows[2][1], "2");
        let page = Report {
            schema_version: 1,
            findings: vec![],
            tables: vec![Table {
                columns: vec!["Dependency".into()],
                rows: vec![vec!["minecraft".into()]],
            }],
        };
        assert_eq!(
            apply_presentation(
                page,
                "en",
                &serde_json::json!({"show_dependencies":false}),
                false
            )
            .tables[0]
                .rows
                .len(),
            1
        );
    }
    #[test]
    fn original_index_reports_declared_values() {
        let report = analyze_bytes(include_bytes!("../fixtures/modrinth.index.json")).unwrap();
        assert_eq!(report.tables[0].rows[2][1], "2");
        assert_eq!(report.tables[0].rows[3][1], "30");
        assert_eq!(report.tables[2].rows[0], vec!["Client", "1", "1", "0"]);
        assert!(report.findings[0].evidence.is_empty());
    }
    #[test]
    fn rejects_zip_and_malformed_json() {
        assert!(matches!(
            analyze_bytes(b"PK\x03\x04"),
            Err(Failure::Unsupported)
        ));
        assert!(matches!(analyze_bytes(b"{}"), Err(Failure::Unsupported)));
    }

    fn snapshot() -> serde_json::Value {
        serde_json::json!({"index":{"formatVersion":1,"game":"minecraft","name":"Fixture","versionId":"1","dependencies":{"minecraft":"1.21.1"},"files":[{"path":"mods/example.jar","fileSize":10,"hashes":{"sha512":"abc"},"downloads":["https://example.invalid/mod.jar"]}]},"overrides":[{"path":"overrides/config/example.txt","size":3,"crc32":7}]})
    }

    #[test]
    fn snapshot_lists_composition_and_complete_file_changes() {
        let baseline = snapshot();
        let mut current = baseline.clone();
        current["index"]["files"][0]["fileSize"] = serde_json::json!(20);
        current["overrides"][0]["crc32"] = serde_json::json!(9);
        let bytes = serde_json::to_vec(&serde_json::json!({"current":current,"baseline":baseline}))
            .unwrap();
        let report = analyze_snapshot(&bytes).unwrap();
        assert_eq!(report.tables[2].rows.len(), 2);
        assert_eq!(report.tables[3].rows.len(), 2);
        assert_eq!(
            &report.tables[3].rows[0][..3],
            ["declared_files", "changed", "mods/example.jar"]
        );
        assert!(report.tables[3].rows[0][3].contains("file_size\":10"));
        assert!(report.tables[3].rows[0][4].contains("file_size\":20"));
        assert!(report.findings.iter().any(|f| f.code == "comparison_basis"));
    }

    #[test]
    fn large_list_explicitly_reports_unshown_rows() {
        let mut current = snapshot();
        current["index"]["files"] = serde_json::Value::Array((0..2100).map(|i| {
            serde_json::json!({"path":format!("mods/{i}.jar"),"fileSize":1,"hashes":{},"downloads":[]})
        }).collect());
        let report =
            analyze_snapshot(&serde_json::to_vec(&serde_json::json!({"current":current})).unwrap())
                .unwrap();
        assert!(report.tables.iter().map(|t| t.rows.len()).sum::<usize>() <= 2048);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == "bounded_listing" && f.message.contains("rows unshown"))
        );
        assert!(
            report.tables[0]
                .rows
                .iter()
                .any(|r| r[1] == "file_count" && r[2] == "2100")
        );
    }

    #[test]
    fn unicode_cells_are_shortened_at_character_boundaries() {
        let mut budget = ListingBudget::default();
        let mut table = Table {
            columns: vec!["Path".into()],
            rows: vec![],
        };
        budget.add(&mut table, vec!["路径".repeat(800)]);
        assert!(table.rows[0][0].len() <= 2048);
        assert!(table.rows[0][0].ends_with("[shortened]"));
        assert_eq!(budget.shortened, 1);
    }

    #[test]
    fn bounded_page_keeps_exact_dataset_totals_and_marks_detail_previews() {
        let envelope = serde_json::json!({"protocol":"cobble.mrpack-page@1","kind":"page","page":{
            "query":{"section":"changes","search":"配置","offset":100,"limit":1},
            "total_rows":50000,"matched_rows":1234,"next_offset":101,"rows":[{
                "id":"r100","source":"declared_file","change":"changed",
                "path":{"text":"配置/example.json","byte_length":19,"complete":true},
                "before":{"text":"old metadata","byte_length":6000,"complete":false},
                "after":{"text":"new metadata","byte_length":7000,"complete":false}}]}});
        let report = analyze_snapshot(&serde_json::to_vec(&envelope).unwrap()).unwrap();
        assert_eq!(report.tables[0].rows[1], vec!["Total rows", "50000"]);
        assert_eq!(report.tables[0].rows[2], vec!["Matching rows", "1234"]);
        assert_eq!(report.tables[1].rows.len(), 1);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "field_details_available")
        );
        assert!(
            !report
                .findings
                .iter()
                .any(|finding| finding.code == "bounded_listing")
        );
        let mut invalid = envelope;
        invalid["protocol"] = serde_json::json!("cobble.mrpack-page@2");
        assert!(matches!(
            analyze_snapshot(&serde_json::to_vec(&invalid).unwrap()),
            Err(Failure::Unsupported)
        ));
    }

    #[test]
    fn detail_envelope_preserves_exact_chunk_text() {
        let text = "配置😀".repeat(100);
        let value = serde_json::json!({"protocol":"cobble.mrpack-page@1","kind":"detail","detail":{
            "row_id":"r100","field":"after","total_bytes":7000,"offset":1000,
            "next_offset":1000+text.len(),"text":text}});
        let report = analyze_snapshot(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(report.tables[0].rows[3][1], text);
    }
}

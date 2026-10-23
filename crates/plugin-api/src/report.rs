// SPDX-License-Identifier: MIT
//! Pure-data reports for the resource-report 0.1.0 world.
use crate::{MAX_REPLY_BYTES, PluginError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema_version: u32,
    pub findings: Vec<Finding>,
    pub tables: Vec<Table>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub evidence: Vec<Evidence>,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub input_index: u32,
    pub first_line: u32,
    pub last_line: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Validate a report against the one immutable input granted to this invocation.
pub fn validate_report(report: &Report, media_type: &str, data: &[u8]) -> Result<(), PluginError> {
    let invalid = || {
        PluginError::new(
            "PLUGIN_INVALID_REPLY",
            "Report exceeds supported limits or contains invalid evidence",
        )
    };
    if report.schema_version != 1 || report.findings.len() > 64 || report.tables.len() > 4 {
        return Err(invalid());
    }
    let text = if media_type == "text/plain" {
        Some(std::str::from_utf8(data).map_err(|_| invalid())?)
    } else {
        None
    };
    let lines = text.map_or(0, |text| text.lines().count());
    let string_valid = |value: &str| value.len() <= 2048;
    for finding in &report.findings {
        if finding.code.len() > 80
            || !string_valid(&finding.title)
            || !string_valid(&finding.message)
            || finding.evidence.len() > 8
            || finding.suggestions.len() > 8
            || finding.suggestions.iter().any(|value| !string_valid(value))
        {
            return Err(invalid());
        }
        for evidence in &finding.evidence {
            if text.is_none()
                || evidence.input_index != 0
                || evidence.first_line == 0
                || evidence.last_line < evidence.first_line
                || evidence.last_line as usize > lines
            {
                return Err(invalid());
            }
        }
    }
    let mut total_rows = 0usize;
    for table in &report.tables {
        total_rows = total_rows
            .checked_add(table.rows.len())
            .ok_or_else(invalid)?;
        if table.columns.len() > 8
            || total_rows > 2048
            || table.columns.iter().any(|value| !string_valid(value))
            || table.rows.iter().any(|row| {
                row.len() != table.columns.len() || row.iter().any(|value| !string_valid(value))
            })
        {
            return Err(invalid());
        }
    }
    let encoded = serde_json::to_vec(report).map_err(|_| invalid())?;
    if encoded.len() > MAX_REPLY_BYTES {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            schema_version: 1,
            findings: vec![Finding {
                code: "java.version".into(),
                severity: Severity::Warning,
                title: "Java version".into(),
                message: "Use the required Java version".into(),
                evidence: vec![Evidence {
                    input_index: 0,
                    first_line: 1,
                    last_line: 2,
                }],
                suggestions: vec!["Choose Java".into()],
            }],
            tables: vec![Table {
                columns: vec!["Name".into()],
                rows: vec![vec!["Example".into()]],
            }],
        }
    }

    #[test]
    fn evidence_is_bound_to_actual_text_input() {
        let mut report = report();
        assert!(validate_report(&report, "text/plain", b"first\r\nsecond\n").is_ok());
        for evidence in [
            Evidence {
                input_index: 1,
                first_line: 1,
                last_line: 2,
            },
            Evidence {
                input_index: 0,
                first_line: 0,
                last_line: 2,
            },
            Evidence {
                input_index: 0,
                first_line: 2,
                last_line: 1,
            },
            Evidence {
                input_index: 0,
                first_line: 1,
                last_line: 3,
            },
        ] {
            report.findings[0].evidence = vec![evidence];
            assert!(validate_report(&report, "text/plain", b"first\nsecond\n").is_err());
        }
        assert!(validate_report(&report, "application/json", b"{}").is_err());
    }

    #[test]
    fn shape_and_utf8_byte_limits_are_enforced() {
        let baseline = report();
        let mut invalid = baseline.clone();
        invalid.tables[0].rows[0].push("extra".into());
        assert!(validate_report(&invalid, "text/plain", b"a\nb").is_err());
        let mut invalid = baseline.clone();
        invalid.findings[0].message = "\u{00e9}".repeat(1025);
        assert!(validate_report(&invalid, "text/plain", b"a\nb").is_err());
        let json = serde_json::to_value(baseline).unwrap();
        assert_eq!(json["findings"][0]["severity"], "warning");
        assert_eq!(json["schema_version"], 1);
    }

    #[test]
    fn total_rows_and_serialized_size_are_bounded() {
        let mut report = Report {
            schema_version: 1,
            findings: vec![],
            tables: vec![Table {
                columns: vec!["Name".into()],
                rows: vec![vec!["x".into()]; 2048],
            }],
        };
        assert!(validate_report(&report, "application/json", b"{}").is_ok());
        report.tables.push(Table {
            columns: vec!["Name".into()],
            rows: vec![vec!["x".into()]],
        });
        assert!(validate_report(&report, "application/json", b"{}").is_err());
        report.tables.pop();
        report.tables[0].rows = vec![vec!["x".repeat(2048)]; 1024];
        assert!(validate_report(&report, "application/json", b"{}").is_err());
    }

    #[test]
    fn version_counts_codes_and_empty_input_evidence_are_rejected() {
        let baseline = report();
        assert!(validate_report(&baseline, "text/plain", b"").is_err());
        assert!(validate_report(&baseline, "text/plain", &[0xff]).is_err());
        for count in 0..7 {
            let mut invalid = baseline.clone();
            match count {
                0 => invalid.schema_version = 2,
                1 => invalid.findings = vec![invalid.findings[0].clone(); 65],
                2 => {
                    invalid.findings[0].evidence = vec![invalid.findings[0].evidence[0].clone(); 9]
                }
                3 => invalid.findings[0].suggestions = vec!["x".into(); 9],
                4 => invalid.findings[0].code = "x".repeat(81),
                5 => invalid.tables = vec![invalid.tables[0].clone(); 5],
                _ => invalid.tables[0].columns = vec!["x".into(); 9],
            }
            assert!(validate_report(&invalid, "text/plain", b"a\nb").is_err());
        }
    }
}

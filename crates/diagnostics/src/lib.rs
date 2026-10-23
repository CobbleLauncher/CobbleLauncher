// SPDX-License-Identifier: MIT
//! Bounded, deterministic analysis of caller-supplied, already redacted log text.
//! No I/O, runtime integration, automatic repair, or general conflict inference.

use serde::{Deserialize, Serialize};

mod redact;

pub use redact::redact_log;

pub const MAX_INPUT_BYTES: usize = 1_048_576;
pub const MAX_EVIDENCE_LINES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassVersion {
    pub required_major: u16,
    pub runtime_major: u16,
    /// Known mapping only; unfamiliar future class versions remain unknown.
    pub required_java: Option<u16>,
    pub runtime_java: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub title_key: String,
    pub message_key: String,
    /// One-based line references, never excerpts or mod names from input.
    pub evidence_lines: Vec<usize>,
    pub suggested_action_keys: Vec<String>,
    pub class_version: Option<ClassVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzeError {
    InputTooLarge {
        maximum_bytes: usize,
        actual_bytes: usize,
    },
}

impl std::fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge {
                maximum_bytes,
                actual_bytes,
            } => write!(
                f,
                "diagnostic input is {actual_bytes} bytes; maximum is {maximum_bytes}"
            ),
        }
    }
}

impl std::error::Error for AnalyzeError {}

/// Class-file majors explicitly supported by this rule set (Java 8 through 25).
pub fn java_for_class_major(major: u16) -> Option<u16> {
    (52..=69).contains(&major).then(|| major - 44)
}

fn major_after(text: &str, marker: &str) -> Option<u16> {
    let tail = text.split_once(marker)?.1;
    let end = tail.find(|c: char| !c.is_ascii_digit())?;
    if end == 0 || !tail[end..].starts_with(".0") {
        return None;
    }
    let rest = &tail[end + 2..];
    if rest.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return None;
    }
    tail[..end].parse().ok()
}

fn record(
    report: &mut Report,
    code: &str,
    line: usize,
    action: &str,
    version: Option<ClassVersion>,
) {
    if let Some(existing) = report
        .findings
        .iter_mut()
        .find(|finding| finding.code == code)
    {
        if existing.class_version == version && existing.evidence_lines.len() < MAX_EVIDENCE_LINES {
            existing.evidence_lines.push(line);
        }
        return;
    }
    report.findings.push(Finding {
        code: code.into(),
        severity: Severity::Error,
        title_key: format!("diagnostics.{code}.title"),
        message_key: format!("diagnostics.{code}.message"),
        evidence_lines: vec![line],
        suggested_action_keys: vec![format!("diagnostics.actions.{action}")],
        class_version: version,
    });
}

/// Analyze at most 1 MiB. Empty findings mean no supported rule matched, not a healthy game.
/// Findings follow first occurrence; repeated rules retain at most eight evidence lines.
pub fn analyze(log: &str) -> Result<Report, AnalyzeError> {
    if log.len() > MAX_INPUT_BYTES {
        return Err(AnalyzeError::InputTooLarge {
            maximum_bytes: MAX_INPUT_BYTES,
            actual_bytes: log.len(),
        });
    }
    let mut report = Report {
        findings: Vec::new(),
    };
    let mut forge_until = 0;
    for (index, line) in log.lines().enumerate() {
        let number = index + 1;
        if line.contains("UnsupportedClassVersionError") {
            let required = major_after(line, "class file version ");
            let runtime = major_after(line, "only recognizes class file versions up to ");
            if let (Some(required_major), Some(runtime_major)) = (required, runtime)
                && required_major > runtime_major
            {
                record(
                    &mut report,
                    "java_class_version",
                    number,
                    "select_compatible_java",
                    Some(ClassVersion {
                        required_major,
                        runtime_major,
                        required_java: java_for_class_major(required_major),
                        runtime_java: java_for_class_major(runtime_major),
                    }),
                );
            }
        }
        if line.contains("java.lang.OutOfMemoryError: Java heap space")
            || line.contains("java.lang.OutOfMemoryError: GC overhead limit exceeded")
        {
            record(
                &mut report,
                "java_heap_exhausted",
                number,
                "review_heap_and_memory_usage",
                None,
            );
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("- Mod '")
            && line.contains(" requires ")
            && line.contains(", which is missing!")
        {
            record(
                &mut report,
                "fabric_required_dependency_missing",
                number,
                "review_required_dependencies",
                None,
            );
        }
        if line.contains("Missing or unsupported mandatory dependencies:") {
            forge_until = number + 16;
        } else if number <= forge_until
            && line.contains("Mod ID:")
            && line.contains("Requested by:")
            && line.contains("Actual version: '[MISSING]'")
        {
            record(
                &mut report,
                "forge_required_dependency_missing",
                number,
                "review_required_dependencies",
                None,
            );
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLASS_ERROR: &str = "java.lang.UnsupportedClassVersionError: synthetic/Main has been compiled by a more recent version of the Java Runtime (class file version 65.0), this version of the Java Runtime only recognizes class file versions up to 61.0";
    const FORGE_HEADER: &str = "[main/ERROR] Missing or unsupported mandatory dependencies:";
    const FORGE_MISSING: &str = "Mod ID: 'synthetic', Requested by: 'fixture', Expected range: '[1,)', Actual version: '[MISSING]'";

    #[test]
    fn extracts_only_supported_numeric_version_metadata() {
        let report = analyze(&format!("secret fixture token\n{CLASS_ERROR}")).unwrap();
        let finding = &report.findings[0];
        assert_eq!(finding.evidence_lines, [2]);
        assert_eq!(
            finding.class_version,
            Some(ClassVersion {
                required_major: 65,
                runtime_major: 61,
                required_java: Some(21),
                runtime_java: Some(17),
            })
        );
        assert!(!format!("{report:?}").contains("secret"));
        assert!(!format!("{report:?}").contains("synthetic/Main"));
    }

    #[test]
    fn does_not_guess_versions_or_generic_mixin_causes() {
        for log in [
            "UnsupportedClassVersionError: unsupported version",
            "MixinApplyError: synthetic injection failed",
            "java.lang.OutOfMemoryError: unable to create native thread",
            "java.lang.OutOfMemoryError: Direct buffer memory",
            "Java heap space",
            "Mod synthetic requires another mod",
            FORGE_MISSING,
        ] {
            assert!(analyze(log).unwrap().findings.is_empty(), "{log}");
        }
        assert!(
            analyze(&CLASS_ERROR.replace("65.0", "61.0"))
                .unwrap()
                .findings
                .is_empty()
        );
        assert!(
            analyze(&CLASS_ERROR.replace("65.0", "65.01"))
                .unwrap()
                .findings
                .is_empty()
        );
    }

    #[test]
    fn future_versions_remain_unknown() {
        let report = analyze(&CLASS_ERROR.replace("65.0", "99.0")).unwrap();
        assert_eq!(
            report.findings[0]
                .class_version
                .as_ref()
                .unwrap()
                .required_java,
            None
        );
        assert_eq!(java_for_class_major(51), None);
        assert_eq!(java_for_class_major(52), Some(8));
        assert_eq!(java_for_class_major(69), Some(25));
        assert_eq!(java_for_class_major(70), None);
    }

    #[test]
    fn recognizes_both_explicit_heap_errors_and_bounds_repeated_evidence() {
        let log = "java.lang.OutOfMemoryError: Java heap space\n".repeat(100);
        let report = analyze(&log).unwrap();
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].evidence_lines,
            (1..=8).collect::<Vec<_>>()
        );
        assert_eq!(
            analyze("Caused by: java.lang.OutOfMemoryError: GC overhead limit exceeded")
                .unwrap()
                .findings
                .len(),
            1
        );
    }

    #[test]
    fn loader_markers_and_bounded_multiline_forge_context() {
        let fabric =
            "  - Mod 'Synthetic' (synthetic) 1 requires any version of fixture, which is missing!";
        let log = format!("{fabric}\r\n{FORGE_HEADER}\r\n{FORGE_MISSING}\r\n{fabric}");
        let report = analyze(&log).unwrap();
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].evidence_lines, [1, 4]);
        assert_eq!(report.findings[1].evidence_lines, [3]);
        let late = format!(
            "{FORGE_HEADER}\n{}{FORGE_MISSING}",
            "unrelated\n".repeat(16)
        );
        assert!(analyze(&late).unwrap().findings.is_empty());
        assert!(
            analyze(&format!(
                "{FORGE_HEADER}\n{}",
                FORGE_MISSING.replace("[MISSING]", "1.0")
            ))
            .unwrap()
            .findings
            .is_empty()
        );
    }

    #[test]
    fn enforces_byte_limit_without_truncating() {
        assert!(
            analyze(&"x".repeat(MAX_INPUT_BYTES))
                .unwrap()
                .findings
                .is_empty()
        );
        assert_eq!(
            analyze(&"x".repeat(MAX_INPUT_BYTES + 1)),
            Err(AnalyzeError::InputTooLarge {
                maximum_bytes: MAX_INPUT_BYTES,
                actual_bytes: MAX_INPUT_BYTES + 1,
            })
        );
        assert!(analyze(&"界".repeat(MAX_INPUT_BYTES / 3 + 1)).is_err());
        assert!(analyze("").unwrap().findings.is_empty());
    }

    #[test]
    fn deterministic_order_and_first_version_evidence() {
        let log = format!(
            "{CLASS_ERROR}\njava.lang.OutOfMemoryError: Java heap space\n{}",
            CLASS_ERROR.replace("65.0", "66.0")
        );
        let first = analyze(&log).unwrap();
        assert_eq!(first, analyze(&log).unwrap());
        assert_eq!(first.findings.len(), 2);
        assert_eq!(first.findings[0].evidence_lines, [1]);
    }
}

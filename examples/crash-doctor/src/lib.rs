// SPDX-License-Identifier: MIT
//! Original report adapter; input logs must already be redacted by the host.
wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin"});
use exports::launcher::resource_report::reports::{Evidence, Finding, Report, Severity, Table};
use launcher::resource_report::resources::{self, Failure, Input};
mod timeline;

fn analyze_bytes(bytes: &[u8]) -> Result<Report, Failure> {
    let text = std::str::from_utf8(bytes).map_err(|_| Failure::Unsupported)?;
    let result = cobble_diagnostics::analyze(text).map_err(|_| Failure::Limit)?;
    let findings = result.findings.into_iter().map(|f| {
        let (title, message, suggestion) = match f.code.as_str() {
            "java_class_version" => ("Java version mismatch", "This game or a Mod needs a newer Java runtime.", "Select a Java runtime compatible with the game and its Mods."),
            "java_heap_exhausted" => ("Java heap exhausted", "Minecraft ran out of its allocated Java memory.", "Review the game's memory setting and memory usage before increasing the limit."),
            _ => ("Required dependency missing", "A Mod required by this installation is missing.", "Review the loader's dependency list and install compatible required Mods."),
        };
        Finding { code:f.code, severity:Severity::Error, title:title.into(), message:message.into(),
            evidence:f.evidence_lines.into_iter().map(|line| Evidence {input_index:0,first_line:line as u32,last_line:line as u32}).collect(), suggestions:vec![suggestion.into()] }
    }).collect();
    Ok(Report {
        schema_version: 1,
        findings,
        tables: timeline::tables(text),
    })
}

fn read_input(input: &Input) -> Result<Vec<u8>, Failure> {
    if resources::cancelled() {
        return Err(Failure::Cancelled);
    }
    let metadata = input.describe()?;
    if metadata.byte_length > cobble_diagnostics::MAX_INPUT_BYTES as u64 {
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
    Ok(bytes)
}
struct CrashDoctor;
impl exports::launcher::resource_report::reports::Guest for CrashDoctor {
    fn analyze(input: &Input) -> Result<Report, Failure> {
        let report = analyze_bytes(&read_input(input)?)?;
        if resources::cancelled() {
            return Err(Failure::Cancelled);
        }
        Ok(report)
    }
}
export!(CrashDoctor);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_fixture_has_exact_evidence() {
        let report = analyze_bytes(include_bytes!("../fixtures/synthetic-crash.txt")).unwrap();
        assert_eq!(report.findings.len(), 2);
        assert_eq!(report.findings[0].code, "java_heap_exhausted");
        assert_eq!(report.findings[0].evidence[0].first_line, 2);
        assert_eq!(report.findings[1].evidence[0].first_line, 3);
        assert!(!report.findings[1].message.contains("Synthetic"));
    }
    #[test]
    fn invalid_utf8_and_limit_fail() {
        assert!(matches!(analyze_bytes(&[255]), Err(Failure::Unsupported)));
        assert!(matches!(
            analyze_bytes(&vec![b'x'; cobble_diagnostics::MAX_INPUT_BYTES + 1]),
            Err(Failure::Limit)
        ));
        assert!(
            analyze_bytes(b"unrecognized failure")
                .unwrap()
                .findings
                .is_empty()
        );
    }
}

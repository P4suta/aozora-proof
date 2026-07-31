//! Deterministic schema-v2 machine report.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::SCHEMA_VERSION;
use crate::finding::{Finding, FixOperation, Position, Span};
use crate::pipeline::Report;
use crate::rules;

/// A path and report pair for multi-file serialization.
#[derive(Debug, Clone, Copy)]
pub struct ReportFile<'a> {
    /// Normalized display path.
    pub path: &'a str,
    /// Proofreading result.
    pub report: &'a Report,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineReport<'a> {
    schema_version: u32,
    tool: ToolWire<'a>,
    summary: SummaryWire,
    files: Vec<FileWire<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolWire<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryWire {
    files: usize,
    findings: usize,
    errors: usize,
    warnings: usize,
    notes: usize,
    conformant_files: usize,
    review_pending_files: usize,
    manual_checks: Vec<ManualCheckWire>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManualCheckWire {
    id: &'static str,
    title: &'static str,
    authority_url: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileWire<'a> {
    path: &'a str,
    encoding: &'static str,
    line_ending: &'static str,
    orthography: &'static str,
    conformant: bool,
    review_pending: bool,
    findings: Vec<FindingWire<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FindingWire<'a> {
    code: &'a str,
    category: &'static str,
    severity: &'static str,
    source: &'static str,
    utf8_byte_span: SpanWire,
    position: PositionWire,
    canonical_message: &'a str,
    data: &'a BTreeMap<String, String>,
    authority_url: &'a str,
    fix_alternatives: Vec<FixWire<'a>>,
}

#[derive(Serialize)]
struct SpanWire {
    start: u32,
    end: u32,
}

impl From<Span> for SpanWire {
    fn from(value: Span) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PositionWire {
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FixWire<'a> {
    applicability: &'static str,
    label: &'a str,
    operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    edit: Option<EditWire<'a>>,
}

#[derive(Serialize)]
struct EditWire<'a> {
    span: SpanWire,
    replacement: &'a str,
}

/// Serialize one in-memory report using the path `<memory>`.
#[must_use]
pub fn serialize_report(report: &Report) -> String {
    serialize_reports(&[ReportFile {
        path: "<memory>",
        report,
    }])
}

/// Serialize ordered file reports as canonical compact JSON.
#[must_use]
pub fn serialize_reports(files: &[ReportFile<'_>]) -> String {
    let file_wires: Vec<FileWire<'_>> = files
        .iter()
        .map(|file| file_wire(file.path, file.report))
        .collect();
    let summary = summary(&file_wires);
    let report = MachineReport {
        schema_version: SCHEMA_VERSION,
        tool: ToolWire {
            name: "aozora-proof",
            version: env!("CARGO_PKG_VERSION"),
        },
        summary,
        files: file_wires,
    };
    serde_json::to_string(&report).unwrap_or_else(|_| {
        String::from(
            r#"{"schemaVersion":2,"tool":{"name":"aozora-proof","version":"0.2.0"},"summary":{"files":0,"findings":0,"errors":0,"warnings":0,"notes":0,"conformantFiles":0,"reviewPendingFiles":0,"manualChecks":[]},"files":[]}"#,
        )
    })
}

fn file_wire<'a>(path: &'a str, report: &'a Report) -> FileWire<'a> {
    FileWire {
        path,
        encoding: report.encoding.as_wire_str(),
        line_ending: report.line_ending.as_wire_str(),
        orthography: report.orthography.as_str(),
        conformant: report.conformant(),
        review_pending: report.review_pending(),
        findings: report
            .findings
            .iter()
            .map(|finding| finding_wire(finding, &report.decoded))
            .collect(),
    }
}

fn finding_wire<'a>(finding: &'a Finding, decoded: &'a str) -> FindingWire<'a> {
    let start = finding.position(decoded);
    let end = crate::finding::position(decoded, finding.span.end);
    FindingWire {
        code: finding.code,
        category: finding.category.as_wire_str(),
        severity: finding.severity.as_wire_str(),
        source: finding.source.as_wire_str(),
        utf8_byte_span: finding.span.into(),
        position: position_wire(start, end),
        canonical_message: &finding.message,
        data: &finding.data,
        authority_url: finding.authority_url,
        fix_alternatives: finding
            .fixes
            .iter()
            .map(|fix| FixWire {
                applicability: fix.applicability.as_wire_str(),
                label: &fix.label,
                operation: fix.operation.as_wire_str(),
                edit: match &fix.operation {
                    FixOperation::Text(edit) => Some(EditWire {
                        span: edit.span.into(),
                        replacement: &edit.replacement,
                    }),
                    FixOperation::RemoveBom
                    | FixOperation::NormalizeCrLf
                    | FixOperation::EnsureFinalNewline
                    | FixOperation::EncodeShiftJis => None,
                },
            })
            .collect(),
    }
}

const fn position_wire(start: Position, end: Position) -> PositionWire {
    PositionWire {
        line: start.line,
        column: start.column,
        end_line: end.line,
        end_column: end.column,
    }
}

fn summary(files: &[FileWire<'_>]) -> SummaryWire {
    let findings = files.iter().flat_map(|file| &file.findings);
    SummaryWire {
        files: files.len(),
        findings: findings.clone().count(),
        errors: findings
            .clone()
            .filter(|finding| finding.severity == "error")
            .count(),
        warnings: findings
            .clone()
            .filter(|finding| finding.severity == "warning")
            .count(),
        notes: findings
            .filter(|finding| finding.severity == "note")
            .count(),
        conformant_files: files.iter().filter(|file| file.conformant).count(),
        review_pending_files: files.iter().filter(|file| file.review_pending).count(),
        manual_checks: rules::official_items()
            .iter()
            .filter(|item| item.detection == crate::DetectionClass::Manual)
            .map(|item| ManualCheckWire {
                id: item.id,
                title: item.title,
                authority_url: item.authority_url,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{Orthography, run_submission_with_orthography};

    use super::*;

    #[test]
    fn schema_v2_contains_file_metadata_and_canonical_message() {
        let report = run_submission_with_orthography("ｱ\n".as_bytes(), Orthography::Modern);
        let json = serialize_report(&report);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(
            value
                .get("schemaVersion")
                .and_then(serde_json::Value::as_u64),
            Some(2)
        );
        let finding = value
            .get("files")
            .and_then(serde_json::Value::as_array)
            .and_then(|files| files.first())
            .and_then(|file| file.get("findings"))
            .and_then(serde_json::Value::as_array)
            .and_then(|findings| findings.first())
            .expect("finding");
        assert!(finding.get("canonicalMessage").is_some());
        assert!(finding.get("utf8ByteSpan").is_some());
        assert!(finding.get("position").is_some());
    }
}

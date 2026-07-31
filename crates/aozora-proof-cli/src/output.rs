use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

use anstyle::{AnsiColor, Style};
use aozora_proof_core::{
    DetectionClass, Finding, FixOperation, ReportFile, Severity, all_rules, official_items,
    serialize_reports,
};

use crate::cli::{ColorChoice, Format, LanguageArg};
use crate::document::Document;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Painter {
    enabled: bool,
}

impl Painter {
    pub(crate) fn resolve(choice: ColorChoice) -> Self {
        let enabled = match choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                io::stdout().is_terminal()
                    && env::var_os("NO_COLOR").is_none()
                    && env::var("TERM").map_or(true, |term| term != "dumb")
            }
        };
        Self { enabled }
    }

    fn paint(self, style: Style, text: &str) -> String {
        if self.enabled {
            format!("{}{text}{}", style.render(), style.render_reset())
        } else {
            text.to_owned()
        }
    }
}

pub(crate) fn resolve_format(format: Format) -> Format {
    match format {
        Format::Auto if io::stdout().is_terminal() => Format::Human,
        Format::Auto => Format::Json,
        explicit => explicit,
    }
}

pub(crate) fn render(
    documents: &[Document],
    format: Format,
    language: LanguageArg,
    painter: Painter,
) -> Vec<u8> {
    match resolve_format(format) {
        Format::Human => human(documents, language, painter).into_bytes(),
        Format::Json => json(documents).into_bytes(),
        Format::Short => short(documents).into_bytes(),
        Format::Sarif => sarif(documents).into_bytes(),
        Format::Auto => Vec::new(),
    }
}

fn json(documents: &[Document]) -> String {
    let files: Vec<ReportFile<'_>> = documents
        .iter()
        .map(|document| ReportFile {
            path: &document.label,
            report: &document.report,
        })
        .collect();
    let mut output = serialize_reports(&files);
    output.push('\n');
    output
}

fn human(documents: &[Document], language: LanguageArg, painter: Painter) -> String {
    let japanese = language == LanguageArg::Ja;
    let mut output = String::new();
    let mut total = 0usize;
    for document in documents {
        let heading = painter.paint(Style::new().bold(), &document.label);
        let _ = writeln!(output, "{heading}:");
        if document.report.findings.is_empty() {
            let clean = if japanese {
                "  自動検査の指摘はありません。"
            } else {
                "  No automated findings."
            };
            let _ = writeln!(output, "{clean}");
        }
        for finding in &document.report.findings {
            let position = finding.position(&document.report.decoded);
            let severity = painter.paint(
                severity_style(finding.severity),
                finding.severity.as_wire_str(),
            );
            let code = painter.paint(Style::new().dimmed(), finding.code);
            let _ = writeln!(
                output,
                "  {}:{}  {:7}  {}  {}",
                position.line,
                position.column,
                severity,
                code,
                finding.localized_message(japanese)
            );
            for fix in &finding.fixes {
                let label = if japanese { &fix.label_ja } else { &fix.label };
                let _ = writeln!(
                    output,
                    "           ↳ {}: {label}",
                    fix.applicability.as_wire_str()
                );
            }
            total += 1;
        }
        let _ = writeln!(output);
    }

    let manual: Vec<_> = official_items()
        .iter()
        .filter(|item| item.detection == DetectionClass::Manual)
        .collect();
    let checklist = if japanese {
        "人手確認（自動確認済みではありません）"
    } else {
        "Manual checks (not automatically verified)"
    };
    let _ = writeln!(output, "{checklist}:");
    for item in manual {
        let title = if japanese { item.title_ja } else { item.title };
        let _ = writeln!(output, "  - {title}  {}", item.authority_url);
    }
    let summary = if japanese {
        format!("{total} 件の指摘。")
    } else {
        format!("{total} finding(s).")
    };
    let _ = writeln!(output, "{summary}");
    output
}

fn short(documents: &[Document]) -> String {
    let mut output = String::new();
    for document in documents {
        for finding in &document.report.findings {
            let position = finding.position(&document.report.decoded);
            let _ = writeln!(
                output,
                "{}:{}:{}: {} {} {}",
                document.label,
                position.line,
                position.column,
                finding.severity.as_wire_str(),
                finding.code,
                finding.message
            );
        }
    }
    output
}

fn sarif(documents: &[Document]) -> String {
    let mut rules = BTreeMap::new();
    let mut results = Vec::new();
    let mut artifacts = Vec::new();
    for document in documents {
        artifacts.push(serde_json::json!({
            "location": { "uri": document.label },
            "encoding": document.report.encoding.as_wire_str(),
        }));
        for finding in &document.report.findings {
            rules
                .entry(finding.code)
                .or_insert_with(|| rule_json(finding));
            results.push(result_json(document, finding));
        }
    }
    let document = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "columnKind": "unicodeCodePoints",
            "tool": {
                "driver": {
                    "name": "aozora-proof",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/P4suta/aozora-proof",
                    "rules": rules.into_values().collect::<Vec<_>>(),
                }
            },
            "artifacts": artifacts,
            "results": results,
        }]
    });
    let mut output = serde_json::to_string(&document).unwrap_or_else(|_| "{}".to_owned());
    output.push('\n');
    output
}

fn rule_json(finding: &Finding) -> serde_json::Value {
    let title = all_rules()
        .iter()
        .find(|rule| rule.code == finding.code)
        .map_or_else(|| finding.kind(), |rule| rule.title);
    serde_json::json!({
        "id": finding.code,
        "name": finding.kind(),
        "shortDescription": { "text": title },
        "helpUri": finding.authority_url,
    })
}

fn result_json(document: &Document, finding: &Finding) -> serde_json::Value {
    let start = finding.position(&document.report.decoded);
    let end = aozora_proof_core::position(&document.report.decoded, finding.span.end);
    let region = serde_json::json!({
        "startLine": start.line,
        "startColumn": start.column,
        "endLine": end.line,
        "endColumn": end.column,
    });
    let fixes: Vec<_> = finding
        .fixes
        .iter()
        .filter_map(|fix| match &fix.operation {
            FixOperation::Text(edit) => {
                let edit_start =
                    aozora_proof_core::position(&document.report.decoded, edit.span.start);
                let edit_end = aozora_proof_core::position(&document.report.decoded, edit.span.end);
                Some(serde_json::json!({
                    "description": { "text": fix.label },
                    "artifactChanges": [{
                        "artifactLocation": { "uri": document.label },
                        "replacements": [{
                            "deletedRegion": {
                                "startLine": edit_start.line,
                                "startColumn": edit_start.column,
                                "endLine": edit_end.line,
                                "endColumn": edit_end.column,
                            },
                            "insertedContent": { "text": edit.replacement },
                        }]
                    }]
                }))
            }
            FixOperation::RemoveBom
            | FixOperation::NormalizeCrLf
            | FixOperation::EnsureFinalNewline
            | FixOperation::EncodeShiftJis => None,
        })
        .collect();
    serde_json::json!({
        "ruleId": finding.code,
        "level": sarif_level(finding.severity),
        "message": { "text": finding.message },
        "locations": [{
            "physicalLocation": {
                "artifactLocation": { "uri": document.label },
                "region": region,
            }
        }],
        "fixes": fixes,
    })
}

fn severity_style(severity: Severity) -> Style {
    let color = match severity {
        Severity::Error => AnsiColor::Red,
        Severity::Warning => AnsiColor::Yellow,
        Severity::Note => AnsiColor::Blue,
    };
    Style::new().bold().fg_color(Some(color.into()))
}

const fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

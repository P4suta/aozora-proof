use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

use anstyle::{AnsiColor, Style};
use aozora_proof_core::{
    CheckError, DetectionClass, Finding, FixOperation, Origin, ReportFile, Severity, all_rules,
    official_items, serialize_reports,
};

use crate::cli::{ColorChoice, Format, LanguageArg};
use crate::document::Document;

#[derive(Debug, thiserror::Error)]
pub(crate) enum RenderError {
    #[error("machine report serialization failed: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("report coordinates are invalid: {source}")]
    Check {
        #[source]
        source: CheckError,
    },
    #[error("in-memory text rendering failed: {source}")]
    Format {
        #[source]
        source: fmt::Error,
    },
    #[error("automatic output format was not resolved")]
    UnresolvedFormat,
}

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
) -> Result<Vec<u8>, RenderError> {
    match resolve_format(format) {
        Format::Human => human(documents, language, painter).map(String::into_bytes),
        Format::Json => json(documents).map(String::into_bytes),
        Format::Short => short(documents).map(String::into_bytes),
        Format::Sarif => sarif(documents).map(String::into_bytes),
        Format::Auto => Err(RenderError::UnresolvedFormat),
    }
}

fn json(documents: &[Document]) -> Result<String, RenderError> {
    let files: Vec<ReportFile<'_>> = documents
        .iter()
        .map(|document| ReportFile {
            path: &document.label,
            report: &document.report,
        })
        .collect();
    let mut output =
        serialize_reports(&files).map_err(|source| RenderError::Serialize { source })?;
    output.push('\n');
    Ok(output)
}

fn human(
    documents: &[Document],
    language: LanguageArg,
    painter: Painter,
) -> Result<String, RenderError> {
    let japanese = language == LanguageArg::Ja;
    let mut output = String::new();
    let mut total = 0usize;
    for document in documents {
        let heading = painter.paint(Style::new().bold(), &document.label);
        writeln!(output, "{heading}:").map_err(|source| RenderError::Format { source })?;
        if document.report.findings.is_empty() {
            let clean = if japanese {
                "  自動検査の指摘はありません。"
            } else {
                "  No automated findings."
            };
            writeln!(output, "{clean}").map_err(|source| RenderError::Format { source })?;
        }
        for finding in &document.report.findings {
            let position = finding
                .position(&document.report.decoded)
                .map_err(|source| RenderError::Check { source })?;
            let severity = painter.paint(
                severity_style(finding.severity),
                finding.severity.as_wire_str(),
            );
            let code = painter.paint(Style::new().dimmed(), finding.code);
            writeln!(
                output,
                "  {}:{}  {:7}  {}  {}",
                position.line,
                position.column,
                severity,
                code,
                finding.localized_message(japanese)
            )
            .map_err(|source| RenderError::Format { source })?;
            for fix in &finding.fixes {
                let label = if japanese { &fix.label_ja } else { &fix.label };
                writeln!(
                    output,
                    "           ↳ {}: {label}",
                    fix.applicability.as_wire_str()
                )
                .map_err(|source| RenderError::Format { source })?;
            }
            total = total.saturating_add(1);
        }
        writeln!(output).map_err(|source| RenderError::Format { source })?;
    }

    let checklist = if japanese {
        "人手確認（自動確認済みではありません）"
    } else {
        "Manual checks (not automatically verified)"
    };
    writeln!(output, "{checklist}:").map_err(|source| RenderError::Format { source })?;
    for item in official_items()
        .iter()
        .filter(|item| item.detection == DetectionClass::Manual)
    {
        let title = if japanese { item.title_ja } else { item.title };
        writeln!(output, "  - {title}  {}", item.authority_url)
            .map_err(|source| RenderError::Format { source })?;
    }
    let summary = if japanese {
        format!("{total} 件の指摘。")
    } else {
        format!("{total} finding(s).")
    };
    writeln!(output, "{summary}").map_err(|source| RenderError::Format { source })?;
    Ok(output)
}

fn short(documents: &[Document]) -> Result<String, RenderError> {
    let mut output = String::new();
    for document in documents {
        for finding in &document.report.findings {
            let position = finding
                .position(&document.report.decoded)
                .map_err(|source| RenderError::Check { source })?;
            writeln!(
                output,
                "{}:{}:{}: {} {} {}",
                document.label,
                position.line,
                position.column,
                finding.severity.as_wire_str(),
                finding.code,
                finding.message
            )
            .map_err(|source| RenderError::Format { source })?;
        }
    }
    Ok(output)
}

fn sarif(documents: &[Document]) -> Result<String, RenderError> {
    let mut rules = BTreeMap::new();
    let mut results = Vec::new();
    let mut artifacts = Vec::new();
    for document in documents {
        artifacts.push(serde_json::json!({
            "location": { "uri": document.label },
            "encoding": document.report.encoding.as_wire_str(),
        }));
        for finding in &document.report.findings {
            if !rules.contains_key(finding.code) {
                rules.insert(finding.code, rule_json(finding)?);
            }
            results.push(result_json(document, finding)?);
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
    let mut output =
        serde_json::to_string(&document).map_err(|source| RenderError::Serialize { source })?;
    output.push('\n');
    Ok(output)
}

fn rule_json(finding: &Finding) -> Result<serde_json::Value, RenderError> {
    let title = match all_rules().iter().find(|rule| rule.code == finding.code) {
        Some(rule) => rule.title,
        None if finding.origin == Origin::Notation => finding.kind(),
        None => {
            return Err(RenderError::Check {
                source: CheckError::UnknownRule { code: finding.code },
            });
        }
    };
    Ok(serde_json::json!({
        "id": finding.code,
        "name": finding.kind(),
        "shortDescription": { "text": title },
        "helpUri": finding.authority_url,
    }))
}

fn result_json(document: &Document, finding: &Finding) -> Result<serde_json::Value, RenderError> {
    let start = finding
        .position(&document.report.decoded)
        .map_err(|source| RenderError::Check { source })?;
    let end = aozora_proof_core::position(&document.report.decoded, finding.span.end)
        .map_err(|source| RenderError::Check { source })?;
    let region = serde_json::json!({
        "startLine": start.line,
        "startColumn": start.column,
        "endLine": end.line,
        "endColumn": end.column,
    });
    let mut fixes = Vec::new();
    for fix in &finding.fixes {
        if let FixOperation::Text(edit) = &fix.operation {
            let edit_start = aozora_proof_core::position(&document.report.decoded, edit.span.start)
                .map_err(|source| RenderError::Check { source })?;
            let edit_end = aozora_proof_core::position(&document.report.decoded, edit.span.end)
                .map_err(|source| RenderError::Check { source })?;
            fixes.push(serde_json::json!({
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
            }));
        }
    }
    Ok(serde_json::json!({
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
    }))
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

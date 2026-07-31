//! Pure orchestration over raw document bytes.

use std::collections::BTreeMap;

use crate::CheckError;
use crate::finding::{
    DetectionClass, Finding, FindingSource, FixOperation, Origin, RuleCategory, Severity, Span,
};
use crate::moji::file_checks::{DetectedEncoding, LineEnding, detect_encoding, detect_line_ending};
use crate::orthography::Orthography;
use crate::rules;

/// Full result for one document.
#[derive(Debug, Clone)]
pub struct Report {
    /// Findings sorted by span and code.
    pub findings: Vec<Finding>,
    /// Decoded text indexed by every finding.
    pub decoded: String,
    /// Detected source encoding.
    pub encoding: DetectedEncoding,
    /// Detected line-ending convention.
    pub line_ending: LineEnding,
    /// Orthography policy used by the run.
    pub orthography: Orthography,
}

impl Report {
    fn new(
        mut findings: Vec<Finding>,
        decoded: String,
        metadata: ReportMetadata,
    ) -> Result<Self, CheckError> {
        for finding in &findings {
            validate_span(&decoded, finding.span)?;
            for fix in &finding.fixes {
                if let FixOperation::Text(edit) = &fix.operation {
                    validate_span(&decoded, edit.span)?;
                }
            }
        }
        findings.sort_by_key(|finding| (finding.span.start, finding.span.end, finding.code));
        Ok(Self {
            findings,
            decoded,
            encoding: metadata.encoding,
            line_ending: metadata.line_ending,
            orthography: metadata.orthography,
        })
    }

    /// Whether every automatically evaluated requirement conforms.
    #[must_use]
    pub fn conformant(&self) -> bool {
        !self.findings.iter().any(|finding| {
            finding.detection == DetectionClass::Automatic
                && matches!(finding.severity, Severity::Error | Severity::Warning)
        })
    }

    /// Whether judgement-dependent work remains visible.
    #[must_use]
    pub fn review_pending(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.detection == DetectionClass::Review)
            || rules::official_items()
                .iter()
                .any(|item| item.detection == DetectionClass::Manual)
    }
}

#[derive(Debug, Clone, Copy)]
struct ReportMetadata {
    encoding: DetectedEncoding,
    line_ending: LineEnding,
    orthography: Orthography,
}

/// Run the upstream notation parser over decoded text.
///
/// # Errors
///
/// Returns [`CheckError`] when the source exceeds the span model, parsing
/// fails, or an upstream diagnostic contains an invalid span.
pub fn run_notation(text: &str) -> Result<Vec<Finding>, CheckError> {
    validate_source_size(text.len())?;
    let document = aozora::parse(text).map_err(|source| CheckError::Parse { source })?;
    document
        .snapshot()
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let span = Span::from(diagnostic.span());
            validate_span(text, span)?;
            Ok(Finding {
                code: diagnostic.code(),
                category: RuleCategory::Notation,
                detection: DetectionClass::Automatic,
                severity: severity_from(diagnostic.severity()),
                origin: Origin::Notation,
                source: source_from(diagnostic.source()),
                span,
                message: diagnostic.to_string(),
                message_ja: diagnostic.to_string(),
                data: BTreeMap::new(),
                authority_url: rules::upstream_rule().authority_url,
                codepoint: None,
                fixes: Vec::new(),
            })
        })
        .collect()
}

/// Run non-submission checks without an orthography direction.
///
/// # Errors
///
/// Returns [`CheckError`] when decoding, parsing, rule lookup, or coordinate
/// validation fails.
pub fn run_all(raw: &[u8]) -> Result<Report, CheckError> {
    run(raw, false, Orthography::Mixed)
}

/// Run submission checks without an orthography direction.
///
/// Interactive and command-line callers should use
/// [`run_submission_with_orthography`] after resolving the required policy.
///
/// # Errors
///
/// Returns [`CheckError`] when decoding, parsing, rule lookup, or coordinate
/// validation fails.
pub fn run_submission(raw: &[u8]) -> Result<Report, CheckError> {
    run(raw, true, Orthography::Mixed)
}

/// Run every submission check under an explicit orthography policy.
///
/// # Errors
///
/// Returns [`CheckError`] when decoding, parsing, rule lookup, or coordinate
/// validation fails.
pub fn run_submission_with_orthography(
    raw: &[u8],
    orthography: Orthography,
) -> Result<Report, CheckError> {
    run(raw, true, orthography)
}

fn run(raw: &[u8], submission: bool, orthography: Orthography) -> Result<Report, CheckError> {
    let encoding = detect_encoding(raw);
    let line_ending = detect_line_ending(raw);
    let decoded = aozora::decode_auto(raw)
        .map_err(|source| CheckError::Decode { source })?
        .into_owned();
    validate_source_size(decoded.len())?;

    let mut findings = crate::moji::file_checks::check(raw)?;
    if submission {
        findings.extend(crate::moji::file_checks::check_submission(raw)?);
    }
    findings.extend(run_notation(&decoded)?);
    findings.extend(crate::moji::check(&decoded)?);
    findings.extend(crate::review::check(&decoded)?);
    findings.extend(crate::kyuji::check(&decoded, orthography)?);
    if submission {
        findings.extend(crate::submission::check(&decoded)?);
    }
    crate::gaiji_dict::annotate(&mut findings)?;

    Report::new(
        findings,
        decoded,
        ReportMetadata {
            encoding,
            line_ending,
            orthography,
        },
    )
}

fn validate_source_size(len: usize) -> Result<(), CheckError> {
    if u32::try_from(len).is_err() {
        return Err(CheckError::SourceTooLarge { len });
    }
    Ok(())
}

fn validate_span(text: &str, span: Span) -> Result<(), CheckError> {
    let start = usize::try_from(span.start).map_err(|source| CheckError::CoordinateConversion {
        byte: span.start,
        source,
    })?;
    let end = usize::try_from(span.end).map_err(|source| CheckError::CoordinateConversion {
        byte: span.end,
        source,
    })?;
    if start > end
        || end > text.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(CheckError::InvalidSpan {
            start: span.start,
            end: span.end,
            source_len: text.len(),
        });
    }
    Ok(())
}

const fn severity_from(severity: aozora::Severity) -> Severity {
    match severity {
        aozora::Severity::Error => Severity::Error,
        aozora::Severity::Note => Severity::Note,
        _ => Severity::Warning,
    }
}

const fn source_from(source: aozora::DiagnosticSource) -> FindingSource {
    match source {
        aozora::DiagnosticSource::Internal => FindingSource::Internal,
        _ => FindingSource::Source,
    }
}

#[cfg(test)]
mod tests {
    use crate::finding::FindingDetails;
    use crate::rules::codes;

    use super::*;

    #[test]
    fn orthography_is_explicit_and_findings_are_stably_sorted() {
        let report = run_submission_with_orthography("來\n".as_bytes(), Orthography::Modern)
            .expect("valid report");
        assert_eq!(report.orthography, Orthography::Modern);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == codes::MODERN_CANDIDATE)
        );
        assert!(report.findings.windows(2).all(|pair| {
            (pair[0].span.start, pair[0].span.end, pair[0].code)
                <= (pair[1].span.start, pair[1].span.end, pair[1].code)
        }));
    }

    #[test]
    fn invalid_encoding_is_a_decode_error() {
        assert!(matches!(
            run_all(&[0xFF, 0xFF, 0xFF]),
            Err(CheckError::Decode { .. })
        ));
    }

    #[test]
    fn source_size_is_rejected_without_allocating_the_source() {
        let oversized = usize::try_from(u32::MAX)
            .expect("u32 fits usize")
            .checked_add(1)
            .expect("usize has room above u32");
        assert!(matches!(
            validate_source_size(oversized),
            Err(CheckError::SourceTooLarge { len }) if len == oversized
        ));
    }

    #[test]
    fn unknown_rule_is_not_treated_as_upstream() {
        let result = Finding::from_rule(
            "aozora::proof::unknown",
            Origin::Character,
            Span { start: 0, end: 0 },
            FindingDetails::new(String::new(), String::new()),
        );
        assert!(matches!(
            result,
            Err(CheckError::UnknownRule {
                code: "aozora::proof::unknown"
            })
        ));
    }

    #[test]
    fn span_overflow_is_explicit() {
        let oversized = usize::try_from(u32::MAX)
            .expect("u32 fits usize")
            .checked_add(1)
            .expect("usize has room above u32");
        assert!(matches!(
            Span::try_from_usize(0, oversized),
            Err(CheckError::SpanOverflow { .. })
        ));
    }
}

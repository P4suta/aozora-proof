//! Pure orchestration over raw document bytes.

use std::collections::BTreeMap;

use crate::finding::{
    DetectionClass, Finding, FindingDetails, FindingSource, Origin, RuleCategory, Severity, Span,
};
use crate::moji::file_checks::{DetectedEncoding, LineEnding, detect_encoding, detect_line_ending};
use crate::orthography::Orthography;
use crate::rules::{self, codes};

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
    /// Construct a deterministically ordered report.
    #[must_use]
    fn new(mut findings: Vec<Finding>, decoded: String, metadata: ReportMetadata) -> Self {
        findings.sort_by_key(|finding| (finding.span.start, finding.span.end, finding.code));
        Self {
            findings,
            decoded,
            encoding: metadata.encoding,
            line_ending: metadata.line_ending,
            orthography: metadata.orthography,
        }
    }

    /// Whether every automatically evaluated requirement conforms.
    #[must_use]
    pub fn conformant(&self) -> bool {
        !self.findings.iter().any(|finding| {
            detection_for(finding) == DetectionClass::Automatic
                && matches!(finding.severity, Severity::Error | Severity::Warning)
        })
    }

    /// Whether judgement-dependent work remains visible.
    #[must_use]
    pub fn review_pending(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| detection_for(finding) == DetectionClass::Review)
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
#[must_use]
pub fn run_notation(text: &str) -> Vec<Finding> {
    let Ok(document) = aozora::parse(text) else {
        return Vec::new();
    };
    document
        .snapshot()
        .diagnostics()
        .iter()
        .map(|diagnostic| Finding {
            code: diagnostic.code(),
            category: RuleCategory::Notation,
            severity: severity_from(diagnostic.severity()),
            origin: Origin::Notation,
            source: source_from(diagnostic.source()),
            span: diagnostic.span().into(),
            message: diagnostic.to_string(),
            message_ja: diagnostic.to_string(),
            data: BTreeMap::new(),
            authority_url: rules::upstream_rule().authority_url,
            codepoint: None,
            fixes: Vec::new(),
        })
        .collect()
}

/// Run non-submission checks without an orthography direction.
#[must_use]
pub fn run_all(raw: &[u8]) -> Report {
    run(raw, false, Orthography::Mixed)
}

/// Run submission checks without an orthography direction.
///
/// Interactive and command-line callers should use
/// [`run_submission_with_orthography`] after resolving the required policy.
#[must_use]
pub fn run_submission(raw: &[u8]) -> Report {
    run(raw, true, Orthography::Mixed)
}

/// Run every submission check under an explicit orthography policy.
#[must_use]
pub fn run_submission_with_orthography(raw: &[u8], orthography: Orthography) -> Report {
    run(raw, true, orthography)
}

fn run(raw: &[u8], submission: bool, orthography: Orthography) -> Report {
    let encoding = detect_encoding(raw);
    let line_ending = detect_line_ending(raw);
    let mut findings = crate::moji::file_checks::check(raw);
    if submission {
        findings.extend(crate::moji::file_checks::check_submission(raw));
    }

    let decoded = if let Ok(text) = aozora::decode_auto(raw) {
        findings.extend(run_notation(&text));
        findings.extend(crate::moji::check(&text));
        findings.extend(crate::review::check(&text));
        findings.extend(crate::kyuji::check(&text, orthography));
        if submission {
            findings.extend(crate::submission::check(&text));
        }
        crate::gaiji_dict::annotate(&mut findings);
        text.into_owned()
    } else {
        findings.push(Finding::from_rule(
            codes::INVALID_ENCODING,
            Origin::Character,
            Span { start: 0, end: 0 },
            FindingDetails::new(
                "The source cannot be decoded as UTF-8 or Shift_JIS.".to_owned(),
                "ファイルを UTF-8 でも Shift_JIS でもデコードできません。".to_owned(),
            ),
        ));
        String::new()
    };

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

fn detection_for(finding: &Finding) -> DetectionClass {
    rules::explain(finding.code).map_or(DetectionClass::Automatic, |rule| rule.detection)
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
    use super::*;

    #[test]
    fn orthography_is_explicit_and_findings_are_stably_sorted() {
        let report = run_submission_with_orthography("來\n".as_bytes(), Orthography::Modern);
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
    fn invalid_encoding_is_an_automatic_error() {
        let report = run_all(&[0xFF, 0xFF, 0xFF]);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == codes::INVALID_ENCODING)
        );
        assert!(!report.conformant());
    }
}

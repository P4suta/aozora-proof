//! Orchestration — combine the notation layer (the `aozora` parser) with the
//! character layers (conformance + 旧字体↔新字体 + 外字注記) into one [`Report`].
//!
//! [`run_all`] runs file-structure checks, the notation layer, the
//! character-conformance and 旧字体↔新字体 layers, and finally a gaiji pass that
//! attaches 外字注記 suggestions to the characters that need one — all over raw
//! bytes.

use crate::coords::SpanMap;
use crate::finding::{Finding, FindingSource, Origin, Severity, Span};

/// The full proofreading result for one document: every finding lifted into
/// the unified DECODED coordinate frame.
#[derive(Debug, Clone)]
pub struct Report {
    /// All findings, sorted by [`crate::Span::start`].
    pub findings: Vec<Finding>,
    /// The decoded source text the findings index into (empty if the input
    /// could not be decoded). Lets callers map byte spans to line/column.
    pub decoded: String,
}

impl Report {
    /// Build a report from an unsorted finding set, sorting by span start.
    #[must_use]
    pub fn new(mut findings: Vec<Finding>, decoded: String) -> Self {
        findings.sort_by_key(|f| (f.span.start, f.span.end));
        Self { findings, decoded }
    }
}

/// Run the notation layer over already-decoded UTF-8 `text`, projecting each
/// `aozora` diagnostic into a unified [`Finding`] in decoded coordinates.
///
/// [`run_all`] adds the character layers on top.
#[must_use]
pub fn run_notation(text: &str) -> Vec<Finding> {
    let map = SpanMap::build(text);
    let doc = aozora::Document::new(text.to_owned());
    let tree = doc.parse();
    tree.diagnostics()
        .iter()
        .map(|d| Finding {
            code: d.code(),
            severity: severity_from(d.severity()),
            origin: Origin::Notation,
            source: source_from(d.source()),
            span: map.map(d.span()),
            message: d.to_string(),
            codepoint: None,
            suggestion: None,
        })
        .collect()
}

/// Run the full proofreading pipeline over raw input bytes.
///
/// File-structure checks run first; then, after decoding, the notation and
/// character layers — all merged and sorted into one [`Report`] in decoded
/// coordinates.
///
/// If the bytes decode as neither UTF-8 nor `Shift_JIS`, only the file-structure
/// findings plus an `invalid_encoding` error are returned (the text layers
/// cannot run on undecodable input).
#[must_use]
pub fn run_all(raw: &[u8]) -> Report {
    let mut findings = crate::moji::file_checks::check(raw);
    let decoded = if let Ok(text) = aozora::encoding::decode_auto(raw) {
        findings.extend(run_notation(&text));
        findings.extend(crate::moji::check(&text));
        findings.extend(crate::kyuji::check(&text));
        // Gaiji layer: enrich (does not add) — attach a 外字注記 suggestion to
        // the character findings that need one. Runs last so it only fills
        // gaps left by the other layers' suggestions.
        crate::gaiji_dict::annotate(&mut findings);
        text.into_owned()
    } else {
        findings.push(Finding {
            code: crate::moji::file_checks::codes::INVALID_ENCODING,
            severity: Severity::Error,
            origin: Origin::Character,
            source: FindingSource::Source,
            span: Span { start: 0, end: 0 },
            message: "ファイルを UTF-8 でも Shift_JIS でもデコードできませんでした。".to_owned(),
            codepoint: None,
            suggestion: None,
        });
        String::new()
    };
    Report::new(findings, decoded)
}

/// Map the parser's `#[non_exhaustive]` severity into our owned enum,
/// defaulting unknown future variants to the visible `Warning`.
const fn severity_from(s: aozora::Severity) -> Severity {
    match s {
        aozora::Severity::Error => Severity::Error,
        aozora::Severity::Note => Severity::Note,
        // `Warning`, plus any future `#[non_exhaustive]` variant, surfaces
        // as the visible `Warning`.
        _ => Severity::Warning,
    }
}

/// Map the parser's `#[non_exhaustive]` diagnostic source into our owned
/// enum, defaulting unknown future variants to `Source`.
const fn source_from(s: aozora::DiagnosticSource) -> FindingSource {
    match s {
        aozora::DiagnosticSource::Internal => FindingSource::Internal,
        // `Source`, plus any future `#[non_exhaustive]` variant, is treated
        // as a user-source issue.
        _ => FindingSource::Source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_all_clean_utf8_is_empty() {
        assert!(
            run_all("青空文庫のふつうの文章。".as_bytes())
                .findings
                .is_empty()
        );
    }

    #[test]
    fn run_all_flags_a_character_issue() {
        let report = run_all("\u{2460}".as_bytes()); // ①
        assert!(report.findings.iter().any(|f| {
            matches!(f.origin, Origin::Character)
                && f.code == crate::moji::codes::PLATFORM_DEPENDENT
        }));
    }

    #[test]
    fn run_all_merges_all_layers_sorted() {
        // run_all must be the union of every layer (file-structure + notation
        // + character) in one sorted report — verified by count so it does not
        // depend on which specific diagnostics the parser emits for an input.
        let raw = "①俱｜青《》".as_bytes();
        let text = std::str::from_utf8(raw).unwrap();
        let expected = crate::moji::file_checks::check(raw).len()
            + run_notation(text).len()
            + crate::moji::check(text).len()
            + crate::kyuji::check(text).len();
        let report = run_all(raw);
        assert_eq!(
            report.findings.len(),
            expected,
            "run_all must merge every layer"
        );
        // The character layer definitely contributed (① 機種依存, 俱 第3水準).
        assert!(
            report
                .findings
                .iter()
                .any(|f| matches!(f.origin, Origin::Character))
        );
        let mut last = 0u32;
        for f in &report.findings {
            assert!(f.span.start >= last, "findings not sorted by span start");
            last = f.span.start;
        }
    }

    #[test]
    fn run_all_reports_invalid_encoding() {
        let report = run_all(&[0xFF, 0xFF, 0xFF]);
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.code == crate::moji::file_checks::codes::INVALID_ENCODING)
        );
    }
}

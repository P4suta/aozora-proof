//! File-structure checks over the **raw** input bytes — BOM presence and
//! line-ending convention.
//!
//! These are document-level findings (their span is the zero-width marker at
//! byte 0), distinct from the per-character scan in the parent module, which
//! works on decoded text.

use aozora::encoding::has_utf8_bom;

use crate::finding::{Finding, FindingSource, Origin, Severity, Span};

/// Stable finding codes for file-structure checks.
pub mod codes {
    /// A UTF-8 BOM is present at the start of the file.
    pub const UTF8_BOM: &str = "aozora::char::utf8_bom";
    /// Line endings are not CR+LF (青空文庫 submission convention).
    pub const CRLF_EXPECTED: &str = "aozora::char::crlf_expected";
    /// The bytes decode as neither UTF-8 nor `Shift_JIS`.
    pub const INVALID_ENCODING: &str = "aozora::char::invalid_encoding";
}

/// Document-level marker span (zero-width at the file start).
const DOC: Span = Span { start: 0, end: 0 };

/// Run file-structure checks over the raw input bytes.
#[must_use]
pub fn check(raw: &[u8]) -> Vec<Finding> {
    let mut findings = Vec::new();

    if has_utf8_bom(raw) {
        findings.push(Finding {
            code: codes::UTF8_BOM,
            severity: Severity::Warning,
            origin: Origin::Character,
            source: FindingSource::Source,
            span: DOC,
            message: "先頭に UTF-8 BOM があります。青空文庫テキストには BOM を含めません。"
                .to_owned(),
            codepoint: None,
            suggestion: None,
        });
    }

    if has_lone_lf(raw) {
        findings.push(Finding {
            code: codes::CRLF_EXPECTED,
            severity: Severity::Note,
            origin: Origin::Character,
            source: FindingSource::Source,
            span: DOC,
            message: "改行が LF です。青空文庫の提出形式は CR+LF（改行コード）です。".to_owned(),
            codepoint: None,
            suggestion: None,
        });
    }

    findings
}

/// True if any LF byte is not immediately preceded by CR — i.e. the file uses
/// (at least partly) bare LF rather than the CR+LF Aozora convention.
fn has_lone_lf(raw: &[u8]) -> bool {
    raw.iter()
        .enumerate()
        .any(|(i, &b)| b == b'\n' && (i == 0 || raw[i - 1] != b'\r'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_utf8_bom() {
        let f = check("\u{feff}あ".as_bytes());
        assert!(f.iter().any(|x| x.code == codes::UTF8_BOM));
    }

    #[test]
    fn lone_lf_noted_but_crlf_clean() {
        assert!(
            check(b"a\nb")
                .iter()
                .any(|x| x.code == codes::CRLF_EXPECTED)
        );
        assert!(
            !check(b"a\r\nb")
                .iter()
                .any(|x| x.code == codes::CRLF_EXPECTED)
        );
    }
}

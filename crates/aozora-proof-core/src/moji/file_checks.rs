//! File-structure checks over the **raw** input bytes.
//!
//! These are document-level findings (their span is the zero-width marker at
//! byte 0), distinct from the per-character scan in the parent module, which
//! works on decoded text.

use aozora::has_utf8_bom;
use std::str::from_utf8;

use crate::finding::{Finding, FindingSource, Origin, Severity, Span};

/// Stable finding codes for file-structure checks.
pub mod codes {
    /// A UTF-8 BOM is present at the start of the file.
    pub const UTF8_BOM: &str = "aozora::char::utf8_bom";
    /// Line endings are not CR+LF (青空文庫 submission convention).
    pub const CRLF_EXPECTED: &str = "aozora::char::crlf_expected";
    /// More than one line-ending convention occurs in the file.
    pub const MIXED_LINE_ENDINGS: &str = "aozora::char::mixed_line_endings";
    /// A non-ASCII source is UTF-8 rather than submission-format Shift_JIS.
    pub const UTF8_SOURCE: &str = "aozora::char::utf8_source";
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

    match line_endings(raw) {
        LineEndings::None | LineEndings::CrLf => {}
        LineEndings::Lf => findings.push(line_ending_finding(
            codes::CRLF_EXPECTED,
            "改行が LF です。青空文庫の提出形式は CR+LF です。",
        )),
        LineEndings::Cr => findings.push(line_ending_finding(
            codes::CRLF_EXPECTED,
            "改行が CR です。青空文庫の提出形式は CR+LF です。",
        )),
        LineEndings::Mixed => findings.push(line_ending_finding(
            codes::MIXED_LINE_ENDINGS,
            "複数の改行形式が混在しています。青空文庫の提出形式は CR+LF です。",
        )),
    }

    findings
}

/// Submission-only encoding checks.
#[must_use]
pub fn check_submission(raw: &[u8]) -> Vec<Finding> {
    if from_utf8(raw).is_ok() && raw.iter().any(|byte| !byte.is_ascii()) {
        vec![Finding {
            code: codes::UTF8_SOURCE,
            severity: Severity::Note,
            origin: Origin::Character,
            source: FindingSource::Source,
            span: DOC,
            message: "入力は UTF-8 です。青空文庫へ提出するファイルは Shift_JIS で保存します。"
                .to_owned(),
            codepoint: None,
            suggestion: None,
        }]
    } else {
        Vec::new()
    }
}

fn line_ending_finding(code: &'static str, message: &str) -> Finding {
    Finding {
        code,
        severity: Severity::Note,
        origin: Origin::Character,
        source: FindingSource::Source,
        span: DOC,
        message: message.to_owned(),
        codepoint: None,
        suggestion: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEndings {
    None,
    CrLf,
    Lf,
    Cr,
    Mixed,
}

fn line_endings(raw: &[u8]) -> LineEndings {
    let mut crlf = false;
    let mut lf = false;
    let mut cr = false;
    let mut index = 0usize;
    while let Some(byte) = raw.get(index) {
        match *byte {
            b'\r' if raw.get(index + 1) == Some(&b'\n') => {
                crlf = true;
                index += 2;
            }
            b'\r' => {
                cr = true;
                index += 1;
            }
            b'\n' => {
                lf = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    match (crlf, lf, cr) {
        (false, false, false) => LineEndings::None,
        (true, false, false) => LineEndings::CrLf,
        (false, true, false) => LineEndings::Lf,
        (false, false, true) => LineEndings::Cr,
        _ => LineEndings::Mixed,
    }
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
    fn pure_line_endings_are_distinguished() {
        assert!(
            check(b"a\nb")
                .iter()
                .any(|x| x.code == codes::CRLF_EXPECTED)
        );
        assert!(
            check(b"a\rb")
                .iter()
                .any(|x| x.code == codes::CRLF_EXPECTED)
        );
        assert!(
            !check(b"a\r\nb")
                .iter()
                .any(|x| x.code == codes::CRLF_EXPECTED)
        );
    }

    #[test]
    fn mixed_line_endings_have_their_own_code() {
        let findings = check(b"a\r\nb\nc\rd");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, codes::MIXED_LINE_ENDINGS);
    }

    #[test]
    fn submission_check_distinguishes_utf8_from_ascii() {
        assert!(
            check_submission("青空".as_bytes())
                .iter()
                .any(|finding| finding.code == codes::UTF8_SOURCE)
        );
        assert!(check_submission(b"ASCII only").is_empty());
    }
}

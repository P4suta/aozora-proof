//! Raw-byte submission checks and detected file metadata.

use std::str::from_utf8;

use aozora::has_utf8_bom;

use crate::CheckError;
use crate::finding::{
    Finding, FindingDetails, FixAlternative, FixApplicability, FixOperation, Origin, Span,
};
use crate::rules::codes;

const DOCUMENT: Span = Span { start: 0, end: 0 };

/// Detected source encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    /// ASCII-only bytes, valid in both supported encodings.
    Ascii,
    /// UTF-8.
    Utf8,
    /// Shift_JIS.
    ShiftJis,
    /// Neither supported encoding.
    Unknown,
}

impl DetectedEncoding {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Ascii => "ascii",
            Self::Utf8 => "utf-8",
            Self::ShiftJis => "shift_jis",
            Self::Unknown => "unknown",
        }
    }
}

/// Detected line-ending convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// No line ending occurs.
    None,
    /// CRLF only.
    CrLf,
    /// LF only.
    Lf,
    /// CR only.
    Cr,
    /// More than one convention.
    Mixed,
}

impl LineEnding {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::CrLf => "crlf",
            Self::Lf => "lf",
            Self::Cr => "cr",
            Self::Mixed => "mixed",
        }
    }
}

/// Detect the source encoding without lossy replacement.
#[must_use]
pub fn detect_encoding(raw: &[u8]) -> DetectedEncoding {
    let without_bom = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    if without_bom.is_ascii() {
        DetectedEncoding::Ascii
    } else if from_utf8(without_bom).is_ok() {
        DetectedEncoding::Utf8
    } else if aozora::decode_auto(raw).is_ok() {
        DetectedEncoding::ShiftJis
    } else {
        DetectedEncoding::Unknown
    }
}

/// Detect the line-ending convention.
#[must_use]
pub fn detect_line_ending(raw: &[u8]) -> LineEnding {
    let mut crlf = false;
    let mut lf = false;
    let mut cr = false;
    let mut bytes = raw.iter().peekable();
    while let Some(byte) = bytes.next() {
        match *byte {
            b'\r' if bytes.peek() == Some(&&b'\n') => {
                crlf = true;
                bytes.next();
            }
            b'\r' => {
                cr = true;
            }
            b'\n' => {
                lf = true;
            }
            _ => {}
        }
    }
    match (crlf, lf, cr) {
        (false, false, false) => LineEnding::None,
        (true, false, false) => LineEnding::CrLf,
        (false, true, false) => LineEnding::Lf,
        (false, false, true) => LineEnding::Cr,
        _ => LineEnding::Mixed,
    }
}

/// Run format checks that apply to every decoded document.
///
/// # Errors
///
/// Returns [`CheckError`] when a required rule is absent from the catalog.
pub fn check(raw: &[u8]) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    if has_utf8_bom(raw) {
        findings.push(file_finding(
            codes::BOM,
            (
                "The source starts with a UTF-8 byte-order mark.",
                "ファイル先頭に UTF-8 BOM があります。",
            ),
            (
                FixOperation::RemoveBom,
                "Remove the byte-order mark",
                "BOM を削除",
            ),
        )?);
    }

    let ending = detect_line_ending(raw);
    if !matches!(ending, LineEnding::None | LineEnding::CrLf) {
        let mut finding = file_finding(
            codes::LINE_ENDING,
            (
                "Line endings are not consistently CRLF.",
                "改行コードが CRLF に統一されていません。",
            ),
            (
                FixOperation::NormalizeCrLf,
                "Normalize line endings to CRLF",
                "改行を CRLF に統一",
            ),
        )?;
        finding
            .data
            .extend([("detected".to_owned(), ending.as_wire_str().to_owned())]);
        findings.push(finding);
    }

    Ok(findings)
}

/// Run checks required specifically for a submission artifact.
///
/// # Errors
///
/// Returns [`CheckError`] when a required rule is absent from the catalog.
pub fn check_submission(raw: &[u8]) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    if detect_encoding(raw) == DetectedEncoding::Utf8 {
        findings.push(file_finding(
            codes::SOURCE_ENCODING,
            (
                "The submission source is UTF-8 rather than Shift_JIS.",
                "提出ファイルが Shift_JIS ではなく UTF-8 です。",
            ),
            (
                FixOperation::EncodeShiftJis,
                "Encode losslessly as Shift_JIS",
                "Shift_JIS へ無損失変換",
            ),
        )?);
    }
    if !raw.is_empty() && !raw.ends_with(b"\n") && !raw.ends_with(b"\r") {
        findings.push(file_finding(
            codes::FINAL_NEWLINE,
            (
                "The document has no final line ending.",
                "文書末尾に改行がありません。",
            ),
            (
                FixOperation::EnsureFinalNewline,
                "Add the final line ending",
                "末尾改行を追加",
            ),
        )?);
    }
    Ok(findings)
}

fn file_finding(
    code: &'static str,
    messages: (&str, &str),
    fix: (FixOperation, &str, &str),
) -> Result<Finding, CheckError> {
    Finding::from_rule(
        code,
        Origin::Character,
        DOCUMENT,
        FindingDetails::new(messages.0.to_owned(), messages.1.to_owned()).with_fixes(vec![
            FixAlternative {
                applicability: FixApplicability::Safe,
                label: fix.1.to_owned(),
                label_ja: fix.2.to_owned(),
                operation: fix.0,
            },
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_encoding_and_line_endings() {
        assert_eq!(detect_encoding("青空".as_bytes()), DetectedEncoding::Utf8);
        assert_eq!(detect_encoding(b"plain"), DetectedEncoding::Ascii);
        assert_eq!(detect_line_ending(b"a\r\nb"), LineEnding::CrLf);
        assert_eq!(detect_line_ending(b"a\r\nb\n"), LineEnding::Mixed);
    }

    #[test]
    fn format_findings_have_safe_operations() {
        let findings = check(b"a\nb").expect("catalog-backed findings");
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == codes::LINE_ENDING)
        );
        assert!(findings.iter().all(|finding| {
            finding
                .fixes
                .iter()
                .all(|fix| fix.applicability == FixApplicability::Safe)
        }));
    }
}

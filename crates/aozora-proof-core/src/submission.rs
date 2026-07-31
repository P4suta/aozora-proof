//! Submission wrapper checks whose correctness still requires review.

use crate::CheckError;
use crate::finding::{Finding, FindingDetails, Origin, Span};
use crate::rules::codes;

/// Check opening and closing submission matter.
///
/// # Errors
///
/// Returns [`CheckError`] when catalog lookup or checked span conversion
/// fails.
pub fn check(text: &str) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    if !text.contains("テキスト中に現れる記号について") {
        findings.push(Finding::from_rule(
            codes::OPENING_LEGEND,
            Origin::Submission,
            Span { start: 0, end: 0 },
            FindingDetails::new(
                "The opening symbol legend was not found.".to_owned(),
                "冒頭の「テキスト中に現れる記号について」が見つかりません。".to_owned(),
            ),
        )?);
    }
    if !text.contains("底本：") {
        let end = Span::try_from_usize(text.len(), text.len())?.end;
        findings.push(Finding::from_rule(
            codes::CLOSING_BIBLIOGRAPHY,
            Origin::Submission,
            Span { start: end, end },
            FindingDetails::new(
                "Closing bibliographical matter was not found.".to_owned(),
                "末尾の「底本：」を含む書誌情報が見つかりません。".to_owned(),
            ),
        )?);
    }
    Ok(findings)
}

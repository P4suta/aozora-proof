//! Conservative review-candidate detectors.

use std::collections::BTreeMap;

use crate::CheckError;
use crate::finding::{Finding, FindingDetails, Origin, Span};
use crate::rules::codes;

/// Find candidates that require comparison with the base edition.
///
/// # Errors
///
/// Returns [`CheckError`] when catalog lookup or checked span arithmetic
/// fails.
pub fn check(text: &str) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    let characters: Vec<(usize, char)> = text.char_indices().collect();
    for (index, &(offset, character)) in characters.iter().enumerate() {
        if matches!(character, 'ケ' | 'ヶ') {
            findings.push(review_finding(
                codes::SMALL_KE,
                span(offset, character)?,
                (
                    format!("Form {character:?} requires confirmation against the base edition."),
                    format!("「{character}」の字体を底本で確認してください。"),
                ),
                character,
            )?);
        }

        let previous = index
            .checked_sub(1)
            .and_then(|position| characters.get(position))
            .map(|&(_, value)| value);
        let next_index = index.checked_add(1).ok_or(CheckError::CoordinateOverflow {
            operation: "advancing a review-candidate index",
        })?;
        let next = characters.get(next_index).map(|&(_, value)| value);
        if suspicious_ocr_context(previous, character, next) {
            findings.push(review_finding(
                codes::OCR_SIMILAR,
                span(offset, character)?,
                (
                    format!(
                        "OCR-confusable character {character:?} occurs in a suspicious script context."
                    ),
                    format!("OCR 類似字「{character}」が不自然な文字種の並びにあります。"),
                ),
                character,
            )?);
        }

        if contextual_ascii(character, previous, next) {
            findings.push(review_finding(
                codes::SPACING,
                span(offset, character)?,
                (
                    format!(
                        "ASCII spacing or punctuation {character:?} occurs in Japanese context."
                    ),
                    format!("和文の文脈に半角空白・記号「{character}」があります。"),
                ),
                character,
            )?);
        }
    }
    findings.extend(ruby_grouping_candidates(text)?);
    Ok(findings)
}

fn suspicious_ocr_context(previous: Option<char>, character: char, next: Option<char>) -> bool {
    const KATAKANA_CONFUSABLE: &str = "タカロエセニトリハオ";
    const KANJI_CONFUSABLE: &str = "夕力口工七二卜一八才";
    (KATAKANA_CONFUSABLE.contains(character)
        && (previous.is_some_and(is_cjk) || next.is_some_and(is_cjk)))
        || (KANJI_CONFUSABLE.contains(character)
            && (previous.is_some_and(is_katakana) || next.is_some_and(is_katakana)))
}

fn contextual_ascii(character: char, previous: Option<char>, next: Option<char>) -> bool {
    match character {
        ' ' => previous.is_some_and(is_japanese) && next.is_some_and(is_japanese),
        '(' | '[' => next.is_some_and(is_japanese),
        ')' | ']' => previous.is_some_and(is_japanese),
        _ => false,
    }
}

fn ruby_grouping_candidates(text: &str) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    for (close, _) in text.match_indices('》') {
        let after_close =
            close
                .checked_add('》'.len_utf8())
                .ok_or(CheckError::CoordinateOverflow {
                    operation: "advancing beyond a ruby delimiter",
                })?;
        let Some(rest) = text.get(after_close..) else {
            continue;
        };
        let line = rest.lines().next().unwrap_or(rest);
        let Some(open) = line.find('《') else {
            continue;
        };
        let Some(base) = line.get(..open) else {
            continue;
        };
        if base.is_empty() || !base.chars().all(is_cjk) {
            continue;
        }
        let start = after_close;
        let end = start
            .checked_add(open)
            .ok_or(CheckError::CoordinateOverflow {
                operation: "computing a ruby-grouping span",
            })?;
        findings.push(Finding::from_rule(
            codes::RUBY_GROUPING,
            Origin::Notation,
            Span::try_from_usize(start, end)?,
            FindingDetails::new(
                "Adjacent ruby groups may be over-divided.".to_owned(),
                "隣接するルビが過分割されている可能性があります。".to_owned(),
            ),
        )?);
    }
    Ok(findings)
}

fn review_finding(
    code: &'static str,
    character_span: Span,
    messages: (String, String),
    character: char,
) -> Result<Finding, CheckError> {
    Finding::from_rule(
        code,
        Origin::Submission,
        character_span,
        FindingDetails::new(messages.0, messages.1)
            .with_data(BTreeMap::from([(
                "character".to_owned(),
                character.to_string(),
            )]))
            .with_codepoint(character),
    )
}

fn span(offset: usize, character: char) -> Result<Span, CheckError> {
    let end = offset
        .checked_add(character.len_utf8())
        .ok_or(CheckError::CoordinateOverflow {
            operation: "computing a review-candidate span",
        })?;
    Span::try_from_usize(offset, end)
}

const fn is_cjk(character: char) -> bool {
    matches!(character, '\u{3400}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}')
}

const fn is_katakana(character: char) -> bool {
    matches!(character, '\u{30A0}'..='\u{30FF}')
}

const fn is_japanese(character: char) -> bool {
    is_cjk(character) || is_katakana(character) || matches!(character, '\u{3040}'..='\u{309F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_are_contextual() {
        assert!(
            check("漢タ字")
                .expect("review check")
                .iter()
                .any(|finding| finding.code == codes::OCR_SIMILAR)
        );
        assert!(
            check("青 空")
                .expect("review check")
                .iter()
                .any(|finding| finding.code == codes::SPACING)
        );
        assert_eq!(
            check("(青空)")
                .expect("review check")
                .iter()
                .filter(|finding| finding.code == codes::SPACING)
                .count(),
            2
        );
        assert!(
            check("一ヶ月")
                .expect("review check")
                .iter()
                .any(|finding| finding.code == codes::SMALL_KE)
        );
    }
}

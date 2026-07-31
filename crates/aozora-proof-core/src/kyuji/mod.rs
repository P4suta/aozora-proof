//! Directional modern and traditional character-form review.

use std::collections::BTreeMap;

use aozora_proof_data::{kyuji_for, shinji_for};

use crate::CheckError;
use crate::finding::{Finding, FindingDetails, FixAlternative, Origin, Span};
use crate::orthography::Orthography;
use crate::rules::codes;

/// Scan text under the selected orthography policy.
///
/// # Errors
///
/// Returns [`CheckError`] when a catalog rule or decoded span is invalid.
pub fn check(text: &str, orthography: Orthography) -> Result<Vec<Finding>, CheckError> {
    match orthography {
        Orthography::Modern => check_modern(text),
        Orthography::Traditional => check_traditional(text),
        Orthography::Mixed => Ok(Vec::new()),
    }
}

fn check_modern(text: &str) -> Result<Vec<Finding>, CheckError> {
    text.char_indices()
        .filter_map(|(offset, character)| {
            let modern = shinji_for(character)?;
            Some((|| {
                let character_span = char_span(offset, character)?;
                Finding::from_rule(
                    codes::MODERN_CANDIDATE,
                    Origin::Orthography,
                    character_span,
                    FindingDetails::new(
                        format!(
                            "Traditional form {character:?} has the modern candidate {modern:?}."
                        ),
                        format!(
                            "旧字体・異体字「{character}」には新字体候補「{modern}」があります。"
                        ),
                    )
                    .with_data(form_data(character, &[modern]))
                    .with_codepoint(character)
                    .with_fixes(vec![FixAlternative::review_text(
                        character_span,
                        modern.to_string(),
                        format!("Use modern form {modern:?}"),
                        format!("新字体「{modern}」を使用"),
                    )]),
                )
            })())
        })
        .collect()
}

fn check_traditional(text: &str) -> Result<Vec<Finding>, CheckError> {
    text.char_indices()
        .filter_map(|(offset, character)| {
            let candidates = kyuji_for(character);
            if candidates.is_empty() {
                return None;
            }
            let character_span = match char_span(offset, character) {
                Ok(span) => span,
                Err(error) => return Some(Err(error)),
            };
            let fixes = candidates
                .iter()
                .map(|candidate| {
                    FixAlternative::review_text(
                        character_span,
                        candidate.to_string(),
                        format!("Use traditional form {candidate:?}"),
                        format!("旧字体候補「{candidate}」を使用"),
                    )
                })
                .collect();
            let joined = candidates.iter().collect::<String>();
            Some(Finding::from_rule(
                codes::TRADITIONAL_CANDIDATE,
                Origin::Orthography,
                character_span,
                FindingDetails::new(
                    format!("Modern form {character:?} has traditional candidates {joined:?}."),
                    format!("新字体「{character}」には旧字体候補「{joined}」があります。"),
                )
                .with_data(form_data(character, &candidates))
                .with_codepoint(character)
                .with_fixes(fixes),
            ))
        })
        .collect()
}

fn char_span(offset: usize, character: char) -> Result<Span, CheckError> {
    let end = offset
        .checked_add(character.len_utf8())
        .ok_or(CheckError::CoordinateOverflow {
            operation: "computing an orthography span",
        })?;
    Span::try_from_usize(offset, end)
}

fn form_data(character: char, candidates: &[char]) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("character".to_owned(), character.to_string()),
        (
            "candidates".to_owned(),
            candidates.iter().collect::<String>(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_are_directional_and_mixed_is_silent() {
        let modern = check("來", Orthography::Modern).expect("modern check");
        assert_eq!(modern.len(), 1);
        assert_eq!(modern[0].code, codes::MODERN_CANDIDATE);

        let traditional = check("来", Orthography::Traditional).expect("traditional check");
        assert_eq!(traditional.len(), 1);
        assert_eq!(traditional[0].code, codes::TRADITIONAL_CANDIDATE);
        assert!(!traditional[0].fixes.is_empty());

        assert!(
            check("來来", Orthography::Mixed)
                .expect("mixed check")
                .is_empty()
        );
    }
}

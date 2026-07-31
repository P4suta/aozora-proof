//! Directional modern and traditional character-form review.

use std::collections::BTreeMap;

use aozora_proof_data::{kyuji_for, shinji_for};

use crate::finding::{Finding, FindingDetails, FixAlternative, Origin, Span};
use crate::orthography::Orthography;
use crate::rules::codes;

/// Scan text under the selected orthography policy.
#[must_use]
pub fn check(text: &str, orthography: Orthography) -> Vec<Finding> {
    match orthography {
        Orthography::Modern => check_modern(text),
        Orthography::Traditional => check_traditional(text),
        Orthography::Mixed => Vec::new(),
    }
}

fn check_modern(text: &str) -> Vec<Finding> {
    text.char_indices()
        .filter_map(|(offset, character)| {
            let modern = shinji_for(character)?;
            let character_span = char_span(offset, character);
            Some(Finding::from_rule(
                codes::MODERN_CANDIDATE,
                Origin::Orthography,
                character_span,
                FindingDetails::new(
                    format!("Traditional form {character:?} has the modern candidate {modern:?}."),
                    format!("旧字体・異体字「{character}」には新字体候補「{modern}」があります。"),
                )
                .with_data(form_data(character, &[modern]))
                .with_codepoint(character)
                .with_fixes(vec![FixAlternative::review_text(
                    character_span,
                    modern.to_string(),
                    format!("Use modern form {modern:?}"),
                    format!("新字体「{modern}」を使用"),
                )]),
            ))
        })
        .collect()
}

fn check_traditional(text: &str) -> Vec<Finding> {
    text.char_indices()
        .filter_map(|(offset, character)| {
            let candidates = kyuji_for(character);
            if candidates.is_empty() {
                return None;
            }
            let character_span = char_span(offset, character);
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

fn char_span(offset: usize, character: char) -> Span {
    Span {
        start: u32::try_from(offset).unwrap_or(u32::MAX),
        end: u32::try_from(offset + character.len_utf8()).unwrap_or(u32::MAX),
    }
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
        let modern = check("來", Orthography::Modern);
        assert_eq!(modern.len(), 1);
        assert_eq!(modern[0].code, codes::MODERN_CANDIDATE);

        let traditional = check("来", Orthography::Traditional);
        assert_eq!(traditional.len(), 1);
        assert_eq!(traditional[0].code, codes::TRADITIONAL_CANDIDATE);
        assert!(!traditional[0].fixes.is_empty());

        assert!(check("來来", Orthography::Mixed).is_empty());
    }
}

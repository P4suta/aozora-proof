//! Character repertoire and canonical-token checks.

pub mod file_checks;

use std::collections::BTreeMap;

use aozora_proof_data::{Suijun, is_platform_dependent, jis_level};

use crate::CheckError;
use crate::finding::{Finding, FindingDetails, FixAlternative, Origin, Span};
use crate::rules::codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharacterIssue {
    PlatformDependent,
    NeedsGaiji,
    Control,
    Tab,
    FormFeed,
}

impl CharacterIssue {
    const fn code(self) -> &'static str {
        match self {
            Self::PlatformDependent => codes::PLATFORM_DEPENDENT,
            Self::NeedsGaiji => codes::NEEDS_GAIJI,
            Self::Control => codes::CONTROL,
            Self::Tab => codes::TAB,
            Self::FormFeed => codes::FORM_FEED,
        }
    }

    const fn origin(self) -> Origin {
        match self {
            Self::Tab | Self::FormFeed => Origin::Submission,
            Self::PlatformDependent | Self::NeedsGaiji | Self::Control => Origin::Character,
        }
    }

    fn messages(self, character: char) -> (String, String) {
        match self {
            Self::PlatformDependent => (
                format!("Platform-dependent character {character:?} is outside JIS X 0208."),
                format!("機種依存文字「{character}」は JIS X 0208 外です。"),
            ),
            Self::NeedsGaiji => (
                format!("Character {character:?} requires an external-character annotation."),
                format!("「{character}」には外字注記が必要です。"),
            ),
            Self::Control => (
                format!(
                    "Control character U+{:04X} cannot occur in submission text.",
                    u32::from(character)
                ),
                format!(
                    "制御文字 U+{:04X} は提出本文に使用できません。",
                    u32::from(character)
                ),
            ),
            Self::Tab => (
                "A tab cannot represent stable source layout.".to_owned(),
                "タブでは底本の配置を安定して表現できません。".to_owned(),
            ),
            Self::FormFeed => (
                "A form feed must be represented by an Aozora page-break annotation.".to_owned(),
                "フォームフィードは青空文庫の改ページ注記で表します。".to_owned(),
            ),
        }
    }
}

fn classify(character: char) -> Option<CharacterIssue> {
    if character == '\t' {
        return Some(CharacterIssue::Tab);
    }
    if character == '\u{000C}' {
        return Some(CharacterIssue::FormFeed);
    }
    if matches!(character, '\u{0000}'..='\u{001F}' | '\u{007F}')
        && !matches!(character, '\r' | '\n')
    {
        return Some(CharacterIssue::Control);
    }
    if character.is_ascii() {
        return None;
    }
    if is_platform_dependent(character) {
        return Some(CharacterIssue::PlatformDependent);
    }
    match jis_level(character) {
        Suijun::Level1 | Suijun::Level2 => None,
        Suijun::Level3 | Suijun::Level4 | Suijun::Outside => Some(CharacterIssue::NeedsGaiji),
    }
}

/// Run character and canonical-token checks over decoded text.
///
/// # Errors
///
/// Returns [`CheckError`] when catalog lookup or checked span arithmetic
/// fails.
pub fn check(text: &str) -> Result<Vec<Finding>, CheckError> {
    let mut findings = Vec::new();
    let mut characters = text.char_indices().peekable();
    while let Some((offset, character)) = characters.next() {
        if let Some(finding) = halfwidth_finding(offset, character, &mut characters)? {
            findings.push(finding);
            continue;
        }

        if let Some(finding) = ruby_marker_finding(text, offset, character)? {
            findings.push(finding);
            continue;
        }

        let rest = text.get(offset..).ok_or(CheckError::DetectorInvariant {
            operation: "reading a character-indexed source suffix",
        })?;
        if let Some(finding) = iteration_finding(rest, offset)? {
            characters.next();
            characters.next();
            findings.push(finding);
            continue;
        }

        if let Some(issue) = classify(character) {
            findings.push(issue_finding(offset, character, issue)?);
        }
    }
    Ok(findings)
}

fn halfwidth_finding(
    offset: usize,
    character: char,
    characters: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<Option<Finding>, CheckError> {
    let Some(mut replacement) = halfwidth_equivalent(character) else {
        return Ok(None);
    };
    let mut end =
        offset
            .checked_add(character.len_utf8())
            .ok_or(CheckError::CoordinateOverflow {
                operation: "computing a half-width kana span",
            })?;
    if let Some(&(_, mark)) = characters.peek()
        && matches!(mark, '\u{FF9E}' | '\u{FF9F}')
        && let Some(composed) = compose_kana(replacement, mark)
    {
        let (_, consumed) = characters.next().ok_or(CheckError::DetectorInvariant {
            operation: "consuming a peeked half-width kana mark",
        })?;
        end = end
            .checked_add(consumed.len_utf8())
            .ok_or(CheckError::CoordinateOverflow {
                operation: "extending a half-width kana span",
            })?;
        replacement = composed;
    }
    let character_span = span(offset, end)?;
    Finding::from_rule(
        codes::HALFWIDTH_KANA,
        Origin::Character,
        character_span,
        FindingDetails::new(
            format!("Half-width kana {character:?} has the full-width equivalent {replacement:?}."),
            format!("半角カナ「{character}」は全角「{replacement}」に対応します。"),
        )
        .with_data(character_data(character))
        .with_codepoint(character)
        .with_fixes(vec![FixAlternative::safe_text(
            character_span,
            replacement.to_string(),
            format!("Replace {character:?} with {replacement:?}"),
            format!("「{character}」を「{replacement}」へ変換"),
        )]),
    )
    .map(Some)
}

fn ruby_marker_finding(
    text: &str,
    offset: usize,
    character: char,
) -> Result<Option<Finding>, CheckError> {
    if character != '|' {
        return Ok(None);
    }
    let end = offset
        .checked_add(character.len_utf8())
        .ok_or(CheckError::CoordinateOverflow {
            operation: "computing a ruby marker span",
        })?;
    let rest = text.get(end..).ok_or(CheckError::DetectorInvariant {
        operation: "reading the suffix after a ruby marker",
    })?;
    if !ruby_boundary_is_unambiguous(rest) {
        return Ok(None);
    }
    let marker_span = span(offset, end)?;
    Finding::from_rule(
        codes::ASCII_RUBY_MARKER,
        Origin::Notation,
        marker_span,
        FindingDetails::new(
            "An ASCII vertical line starts a parser-recognizable ruby base.".to_owned(),
            "被ルビ文字列の区切りに半角縦線が使われています。".to_owned(),
        )
        .with_codepoint(character)
        .with_fixes(vec![FixAlternative::safe_text(
            marker_span,
            "｜".to_owned(),
            "Replace the ASCII boundary marker".to_owned(),
            "区切り記号を全角へ変換".to_owned(),
        )]),
    )
    .map(Some)
}

fn iteration_finding(rest: &str, offset: usize) -> Result<Option<Finding>, CheckError> {
    let Some(bad) = ["／〃＼", "／“＼", "／”＼"]
        .into_iter()
        .find(|candidate| rest.starts_with(candidate))
    else {
        return Ok(None);
    };
    let end = offset
        .checked_add(bad.len())
        .ok_or(CheckError::CoordinateOverflow {
            operation: "computing an iteration-mark span",
        })?;
    let iteration_span = span(offset, end)?;
    Finding::from_rule(
        codes::ITERATION_MARK,
        Origin::Notation,
        iteration_span,
        FindingDetails::new(
            "The voiced double iteration mark uses a non-canonical middle character.".to_owned(),
            "濁点付き二倍踊り字の中央記号が規定形ではありません。".to_owned(),
        )
        .with_fixes(vec![FixAlternative::safe_text(
            iteration_span,
            "／″＼".to_owned(),
            "Use the canonical double-prime form".to_owned(),
            "規定の「／″＼」へ変換".to_owned(),
        )]),
    )
    .map(Some)
}

fn issue_finding(
    offset: usize,
    character: char,
    issue: CharacterIssue,
) -> Result<Finding, CheckError> {
    let end = offset
        .checked_add(character.len_utf8())
        .ok_or(CheckError::CoordinateOverflow {
            operation: "computing a character-issue span",
        })?;
    let issue_span = span(offset, end)?;
    let (message, message_ja) = issue.messages(character);
    Finding::from_rule(
        issue.code(),
        issue.origin(),
        issue_span,
        FindingDetails::new(message, message_ja)
            .with_data(character_data(character))
            .with_codepoint(character)
            .with_fixes(review_fixes(issue, issue_span)),
    )
}

fn review_fixes(issue: CharacterIssue, issue_span: Span) -> Vec<FixAlternative> {
    match issue {
        CharacterIssue::FormFeed => vec![FixAlternative::review_text(
            issue_span,
            "［＃改ページ］".to_owned(),
            "Represent as a page-break annotation".to_owned(),
            "改ページ注記へ置換".to_owned(),
        )],
        CharacterIssue::Tab => vec![FixAlternative::review_text(
            issue_span,
            "［＃ここから1字下げ］".to_owned(),
            "Represent as an indentation annotation".to_owned(),
            "字下注記へ置換".to_owned(),
        )],
        CharacterIssue::PlatformDependent
        | CharacterIssue::NeedsGaiji
        | CharacterIssue::Control => Vec::new(),
    }
}

fn span(start: usize, end: usize) -> Result<Span, CheckError> {
    Span::try_from_usize(start, end)
}

fn character_data(character: char) -> BTreeMap<String, String> {
    BTreeMap::from([(
        "codepoint".to_owned(),
        format!("U+{:04X}", u32::from(character)),
    )])
}

fn ruby_boundary_is_unambiguous(rest: &str) -> bool {
    let line = rest.lines().next().unwrap_or(rest);
    line.find('《').is_some_and(|end| end > 0)
}

fn halfwidth_equivalent(character: char) -> Option<char> {
    const FULLWIDTH: &str = "。「」、・ヲァィゥェォャュョッーアイウエオカキクケコサシスセソタチツテトナニヌネノハヒフヘホマミムメモヤユヨラリルレロワン゛゜";
    let index = u32::from(character).checked_sub(0xFF61)?;
    usize::try_from(index)
        .ok()
        .and_then(|position| FULLWIDTH.chars().nth(position))
}

const fn compose_kana(base: char, mark: char) -> Option<char> {
    match (base, mark) {
        ('ウ', '\u{FF9E}') => Some('ヴ'),
        ('カ', '\u{FF9E}') => Some('ガ'),
        ('キ', '\u{FF9E}') => Some('ギ'),
        ('ク', '\u{FF9E}') => Some('グ'),
        ('ケ', '\u{FF9E}') => Some('ゲ'),
        ('コ', '\u{FF9E}') => Some('ゴ'),
        ('サ', '\u{FF9E}') => Some('ザ'),
        ('シ', '\u{FF9E}') => Some('ジ'),
        ('ス', '\u{FF9E}') => Some('ズ'),
        ('セ', '\u{FF9E}') => Some('ゼ'),
        ('ソ', '\u{FF9E}') => Some('ゾ'),
        ('タ', '\u{FF9E}') => Some('ダ'),
        ('チ', '\u{FF9E}') => Some('ヂ'),
        ('ツ', '\u{FF9E}') => Some('ヅ'),
        ('テ', '\u{FF9E}') => Some('デ'),
        ('ト', '\u{FF9E}') => Some('ド'),
        ('ハ', '\u{FF9E}') => Some('バ'),
        ('ヒ', '\u{FF9E}') => Some('ビ'),
        ('フ', '\u{FF9E}') => Some('ブ'),
        ('ヘ', '\u{FF9E}') => Some('ベ'),
        ('ホ', '\u{FF9E}') => Some('ボ'),
        ('ハ', '\u{FF9F}') => Some('パ'),
        ('ヒ', '\u{FF9F}') => Some('ピ'),
        ('フ', '\u{FF9F}') => Some('プ'),
        ('ヘ', '\u{FF9F}') => Some('ペ'),
        ('ホ', '\u{FF9F}') => Some('ポ'),
        ('ワ', '\u{FF9E}') => Some('ヷ'),
        ('ヲ', '\u{FF9E}') => Some('ヺ'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{FixApplicability, FixOperation};

    use super::*;

    #[test]
    fn halfwidth_pairs_have_one_safe_composed_fix() {
        let findings = check("ｶﾞ").expect("character check");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, codes::HALFWIDTH_KANA);
        let fix = findings[0].fixes.first().expect("safe fix");
        assert_eq!(fix.applicability, FixApplicability::Safe);
        assert!(matches!(fix.operation, FixOperation::Text(_)));
        if let FixOperation::Text(edit) = &fix.operation {
            assert_eq!(edit.replacement, "ガ");
            assert_eq!(edit.span, Span { start: 0, end: 6 });
        }
    }

    #[test]
    fn safe_notation_spellings_are_detected() {
        let findings = check("|青空《あおぞら》／〃＼").expect("character check");
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == codes::ASCII_RUBY_MARKER)
        );
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == codes::ITERATION_MARK)
        );
    }

    #[test]
    fn clean_character_text_has_no_findings() {
        assert!(
            check("青空文庫のふつうの文章。亜")
                .expect("character check")
                .is_empty()
        );
    }
}

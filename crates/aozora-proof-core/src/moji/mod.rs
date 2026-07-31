//! Character-level conformance checks.
//!
//! Walks decoded text and flags any character that may not appear literally
//! in conformant 青空文庫 text. A single priority cascade classifies each
//! character once (so e.g. `①` is reported as 機種依存, not also as 第3水準),
//! emitting `aozora::char::*` findings in decoded byte coordinates:
//!
//! 1. controls — tabs and form feeds receive actionable codes;
//! 2. half-width kana letters and punctuation (JIS X 0201);
//! 3. 機種依存文字 (CP932 ∖ JIS X 0208);
//! 4. 第3/第4水準 and characters outside JIS X 0213.
//!
//! ASCII is intentionally not flagged here (half/full-width handling is a
//! separate concern). File-structure checks (BOM, line endings, encoding)
//! live in [`file_checks`].

pub mod file_checks;

use aozora_proof_data::{Suijun, is_platform_dependent, jis_level};

use crate::finding::{Finding, FindingSource, Origin, Severity, Span};

/// Stable finding codes for the character checker.
pub mod codes {
    /// Half-width katakana (JIS X 0201) used where full-width is required.
    pub const HALFWIDTH_KATAKANA: &str = "aozora::char::halfwidth_katakana";
    /// Half-width punctuation from the JIS X 0201 kana block.
    pub const HALFWIDTH_KANA_PUNCTUATION: &str = "aozora::char::halfwidth_kana_punctuation";
    /// 機種依存文字 — encodable in CP932 but outside JIS X 0208.
    pub const PLATFORM_DEPENDENT: &str = "aozora::char::platform_dependent";
    /// JIS X 0213 第3/第4水準 — representable only via 外字注記.
    pub const NEEDS_GAIJI_CHUKI: &str = "aozora::char::needs_gaiji_chuki";
    /// Outside JIS X 0213 entirely.
    pub const NOT_IN_JISX0213: &str = "aozora::char::not_in_jisx0213";
    /// A forbidden C0/DEL control character.
    pub const CONTROL_CHARACTER: &str = "aozora::char::control_character";
    /// A literal tab character.
    pub const TAB_CHARACTER: &str = "aozora::char::tab_character";
    /// A literal form-feed character.
    pub const FORM_FEED_CHARACTER: &str = "aozora::char::form_feed_character";
}

/// The single classification a non-conformant character receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharIssue {
    HalfwidthKatakana,
    HalfwidthKanaPunctuation,
    PlatformDependent,
    NeedsGaijiChuki,
    NotInJisX0213,
    ControlCharacter,
    TabCharacter,
    FormFeedCharacter,
}

impl CharIssue {
    const fn code(self) -> &'static str {
        match self {
            Self::HalfwidthKatakana => codes::HALFWIDTH_KATAKANA,
            Self::HalfwidthKanaPunctuation => codes::HALFWIDTH_KANA_PUNCTUATION,
            Self::PlatformDependent => codes::PLATFORM_DEPENDENT,
            Self::NeedsGaijiChuki => codes::NEEDS_GAIJI_CHUKI,
            Self::NotInJisX0213 => codes::NOT_IN_JISX0213,
            Self::ControlCharacter => codes::CONTROL_CHARACTER,
            Self::TabCharacter => codes::TAB_CHARACTER,
            Self::FormFeedCharacter => codes::FORM_FEED_CHARACTER,
        }
    }

    const fn severity(self) -> Severity {
        match self {
            Self::HalfwidthKatakana
            | Self::HalfwidthKanaPunctuation
            | Self::PlatformDependent
            | Self::ControlCharacter
            | Self::TabCharacter
            | Self::FormFeedCharacter => Severity::Error,
            Self::NeedsGaijiChuki | Self::NotInJisX0213 => Severity::Warning,
        }
    }

    fn message(self, c: char) -> String {
        match self {
            Self::HalfwidthKatakana => {
                format!("半角カタカナ「{c}」は使用できません。全角に変換してください。")
            }
            Self::HalfwidthKanaPunctuation => {
                format!("半角カナ用約物「{c}」は使用できません。対応する全角記号に変換してください。")
            }
            Self::PlatformDependent => format!(
                "機種依存文字「{c}」は使用できません。外字注記（※［＃…］）に置き換えてください。"
            ),
            Self::NeedsGaijiChuki => {
                format!("「{c}」は JIS X 0208 外（第3・第4水準）です。外字注記が必要です。")
            }
            Self::NotInJisX0213 => {
                format!("「{c}」は JIS X 0213 にありません。外字注記または代替表記が必要です。")
            }
            Self::ControlCharacter => {
                format!("制御文字 U+{:04X} は本文に使用できません。", u32::from(c))
            }
            Self::TabCharacter => {
                "タブ文字は本文に使用できません。底本の配置を確認し、文字または青空文庫注記で表現してください。"
                    .to_owned()
            }
            Self::FormFeedCharacter => {
                "改ページ制御文字は本文に使用できません。［＃改ページ］などの青空文庫注記で表現してください。"
                    .to_owned()
            }
        }
    }
}

/// Classify a single character, or `None` if it is conformant (or ASCII).
fn classify(c: char) -> Option<CharIssue> {
    if c == '\t' {
        return Some(CharIssue::TabCharacter);
    }
    if c == '\u{000C}' {
        return Some(CharIssue::FormFeedCharacter);
    }
    if matches!(c, '\u{0000}'..='\u{001F}' | '\u{007F}') && !matches!(c, '\r' | '\n') {
        return Some(CharIssue::ControlCharacter);
    }
    if c.is_ascii() {
        return None;
    }
    if ('\u{FF61}'..='\u{FF65}').contains(&c) {
        return Some(CharIssue::HalfwidthKanaPunctuation);
    }
    if ('\u{FF66}'..='\u{FF9F}').contains(&c) {
        return Some(CharIssue::HalfwidthKatakana);
    }
    if is_platform_dependent(c) {
        return Some(CharIssue::PlatformDependent);
    }
    match jis_level(c) {
        Suijun::Level1 | Suijun::Level2 => None,
        Suijun::Level3 | Suijun::Level4 => Some(CharIssue::NeedsGaijiChuki),
        Suijun::Outside => Some(CharIssue::NotInJisX0213),
    }
}

/// Run the character-level checks over decoded UTF-8 `text`, returning findings
/// in decoded byte coordinates.
#[must_use]
pub fn check(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (offset, c) in text.char_indices() {
        if let Some(issue) = classify(c) {
            let start = u32::try_from(offset).unwrap_or(u32::MAX);
            let end = u32::try_from(offset + c.len_utf8()).unwrap_or(u32::MAX);
            findings.push(Finding {
                code: issue.code(),
                severity: issue.severity(),
                origin: Origin::Character,
                source: FindingSource::Source,
                span: Span { start, end },
                message: issue.message(c),
                codepoint: Some(c),
                suggestion: None,
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_halfwidth_katakana() {
        let f = check("\u{FF71}");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, codes::HALFWIDTH_KATAKANA);
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn distinguishes_halfwidth_kana_punctuation() {
        let findings = check("\u{FF62}\u{FF65}\u{FF63}");
        assert_eq!(findings.len(), 3);
        assert!(
            findings
                .iter()
                .all(|finding| finding.code == codes::HALFWIDTH_KANA_PUNCTUATION)
        );
    }

    #[test]
    fn flags_platform_dependent_over_gaiji() {
        let f = check("\u{2460}"); // ①
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, codes::PLATFORM_DEPENDENT);
        assert_eq!(f[0].severity, Severity::Error);
    }

    #[test]
    fn flags_third_level_kanji() {
        let f = check("\u{4FF1}"); // 俱 第3水準
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, codes::NEEDS_GAIJI_CHUKI);
        assert_eq!(f[0].severity, Severity::Warning);
    }

    #[test]
    fn distinguishes_actionable_control_characters() {
        let findings = check("a\tb\u{000C}c\u{007F}d\r\n");
        assert_eq!(findings.len(), 3);
        assert_eq!(findings[0].code, codes::TAB_CHARACTER);
        assert_eq!(findings[1].code, codes::FORM_FEED_CHARACTER);
        assert_eq!(findings[2].code, codes::CONTROL_CHARACTER);
    }

    #[test]
    fn clean_text_has_no_findings() {
        assert!(check("青空文庫のふつうの文章。亜").is_empty());
    }

    #[test]
    fn span_and_codepoint_are_correct() {
        // "あ①": あ is 3 bytes (0..3), ① is 3 bytes (3..6).
        let f = check("あ\u{2460}");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].span, Span { start: 3, end: 6 });
        assert_eq!(f[0].codepoint, Some('\u{2460}'));
    }

    #[test]
    fn notation_markers_are_not_flagged() {
        // ｜ ＃ ［ ］ are full-width-alias JIS cells; misclassifying them
        // would flag every ruby / annotation marker in real text.
        assert!(check("｜青空《あおぞら》").is_empty());
        assert!(check("※［＃「青」に傍点］").is_empty());
    }
}

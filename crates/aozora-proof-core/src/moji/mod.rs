//! Character-level conformance checks.
//!
//! Walks decoded text and flags any character that may not appear literally
//! in conformant 青空文庫 text. A single priority cascade classifies each
//! character once (so e.g. `①` is reported as 機種依存, not also as 第3水準),
//! emitting `aozora::char::*` findings in decoded byte coordinates:
//!
//! 1. half-width katakana (JIS X 0201) — banned outright;
//! 2. 機種依存文字 (CP932 ∖ JIS X 0208) — non-portable, needs 外字注記;
//! 3. 第3/第4水準 (JIS X 0213) — needs 外字注記;
//! 4. outside JIS X 0213 entirely — needs 外字注記 / substitute.
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
    /// 機種依存文字 — encodable in CP932 but outside JIS X 0208.
    pub const PLATFORM_DEPENDENT: &str = "aozora::char::platform_dependent";
    /// JIS X 0213 第3/第4水準 — representable only via 外字注記.
    pub const NEEDS_GAIJI_CHUKI: &str = "aozora::char::needs_gaiji_chuki";
    /// Outside JIS X 0213 entirely.
    pub const NOT_IN_JISX0213: &str = "aozora::char::not_in_jisx0213";
}

/// The single classification a non-conformant character receives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharIssue {
    HalfwidthKatakana,
    PlatformDependent,
    NeedsGaijiChuki,
    NotInJisX0213,
}

impl CharIssue {
    const fn code(self) -> &'static str {
        match self {
            Self::HalfwidthKatakana => codes::HALFWIDTH_KATAKANA,
            Self::PlatformDependent => codes::PLATFORM_DEPENDENT,
            Self::NeedsGaijiChuki => codes::NEEDS_GAIJI_CHUKI,
            Self::NotInJisX0213 => codes::NOT_IN_JISX0213,
        }
    }

    const fn severity(self) -> Severity {
        match self {
            // Outright bans / non-portable: hard errors.
            Self::HalfwidthKatakana | Self::PlatformDependent => Severity::Error,
            // Representable, but only as 外字注記: warn.
            Self::NeedsGaijiChuki | Self::NotInJisX0213 => Severity::Warning,
        }
    }

    fn message(self, c: char) -> String {
        match self {
            Self::HalfwidthKatakana => {
                format!("半角カタカナ「{c}」は使用できません。全角に変換してください。")
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
        }
    }
}

/// Classify a single character, or `None` if it is conformant (or ASCII).
fn classify(c: char) -> Option<CharIssue> {
    if c.is_ascii() {
        return None;
    }
    // Half-width katakana (U+FF61..=U+FF9F) — checked before the CP932 test,
    // since these also round-trip through Shift_JIS.
    if ('\u{FF61}'..='\u{FF9F}').contains(&c) {
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
        let f = check("\u{FF71}"); // ｱ
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, codes::HALFWIDTH_KATAKANA);
        assert_eq!(f[0].severity, Severity::Error);
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

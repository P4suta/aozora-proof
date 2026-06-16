//! Old-/new-form kanji (旧字体↔新字体) detection and replacement suggestions.
//!
//! For each 旧字体 / 異体字 character that has a recorded 新字体 counterpart
//! (from the 常用漢字表-derived table in `aozora-proof-data`), emits an advisory
//! `aozora::kyuji::*` finding carrying a [`crate::Suggestion`]. Suggestion-only
//! by design — whether to apply it depends on the 底本 (source edition), so the
//! finding is a `Note` (it never fails CI on its own).

use aozora_proof_data::shinji_for;

use crate::finding::{Finding, FindingSource, Origin, Severity, Span, Suggestion};

/// Stable finding codes for the old-/new-form checker.
pub mod codes {
    /// A 旧字体 / 異体字 with a recorded 新字体 counterpart.
    pub const HAS_SHINJI_FORM: &str = "aozora::kyuji::has_shinji_form";
}

/// Scan decoded text for 旧字体 / 異体字 characters that have a 新字体
/// counterpart, returning one advisory finding (with a replacement suggestion)
/// per occurrence, in decoded byte coordinates.
#[must_use]
pub fn check(text: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (offset, c) in text.char_indices() {
        if let Some(shinji) = shinji_for(c) {
            let start = u32::try_from(offset).unwrap_or(u32::MAX);
            let end = u32::try_from(offset + c.len_utf8()).unwrap_or(u32::MAX);
            let span = Span { start, end };
            findings.push(Finding {
                code: codes::HAS_SHINJI_FORM,
                severity: Severity::Note,
                origin: Origin::Kyuji,
                source: FindingSource::Source,
                span,
                message: format!(
                    "「{c}」は旧字体・異体字です。新字体「{shinji}」に対応します（底本に従って確認してください）。"
                ),
                codepoint: Some(c),
                suggestion: Some(Suggestion {
                    replacement: shinji.to_string(),
                    span,
                    label: format!("旧字体「{c}」→ 新字体「{shinji}」"),
                }),
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_old_form_with_suggestion() {
        let f = check("\u{4F86}"); // 來
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].code, codes::HAS_SHINJI_FORM);
        assert!(matches!(f[0].origin, Origin::Kyuji));
        assert_eq!(f[0].severity, Severity::Note);
        assert_eq!(f[0].suggestion.as_ref().unwrap().replacement, "\u{6765}"); // 来
    }

    #[test]
    fn clean_text_has_no_kyuji_findings() {
        assert!(check("国の文章。").is_empty());
    }
}

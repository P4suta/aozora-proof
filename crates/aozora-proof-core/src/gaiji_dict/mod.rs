//! Gaiji (外字) lookup and search.
//!
//! Bidirectional access over the JIS X 0213 面区点 table and the CC0 外字注記
//! dictionary: character ⇔ 面区点 ⇔ Unicode, plus description search and a
//! suggested 外字注記 form. Backed by `aozora-proof-data`.

use aozora_proof_data::{
    MenKuTen, Suijun, char_at_men_ku_ten, gaiji_descriptions, gaiji_search, men_ku_ten,
};

use crate::finding::{Finding, Span, Suggestion};

/// Everything known about a character that is relevant to 外字注記.
#[derive(Debug, Clone)]
pub struct GaijiInfo {
    /// The character itself.
    pub character: char,
    /// Its Unicode scalar value.
    pub codepoint: u32,
    /// Its JIS X 0213 面区点 position + 水準, if it is a JIS X 0213 character.
    pub men_ku_ten: Option<MenKuTen>,
    /// 外字注記 descriptions recorded for it (may be empty).
    pub descriptions: Vec<&'static str>,
    /// A suggested 外字注記 string, when the character needs one (第3/第4水準).
    pub chuki: Option<String>,
}

/// Look up everything known about `c`.
#[must_use]
pub fn lookup(c: char) -> GaijiInfo {
    let mkt = men_ku_ten(c);
    let descriptions = gaiji_descriptions(c);
    let chuki = mkt.and_then(|m| chuki_form(m, descriptions.first().copied()));
    GaijiInfo {
        character: c,
        codepoint: u32::from(c),
        men_ku_ten: mkt,
        descriptions,
        chuki,
    }
}

/// The character at a JIS X 0213 面区点 position, if the cell is assigned.
#[must_use]
pub fn from_men_ku_ten(men: u8, ku: u8, ten: u8) -> Option<char> {
    char_at_men_ku_ten(men, ku, ten)
}

/// Search 外字注記 descriptions for `query` (substring), returning
/// (description, character) pairs.
#[must_use]
pub fn search(query: &str) -> Vec<(&'static str, char)> {
    gaiji_search(query)
}

/// Enrich character-level findings in place with a suggested 外字注記.
///
/// Walks `findings` and, for each one that is about a single character
/// (`codepoint` set) and does not already carry a [`Suggestion`], attaches the
/// 外字注記 form for that character when a well-formed one exists — a 第3/第4水準
/// cell that also has a recorded description, so the offered 注記 is
/// `※［＃「…」、第N水準 m-k-t］` rather than an empty `「」` stub.
///
/// This is the gaiji *check layer* wired into [`crate::run_all`]: it adds no
/// findings of its own (so it can never double-report against the
/// [`crate::moji`] 外字注記 warning), it only turns a bare "needs 外字注記"
/// finding into an actionable, `--fix`-applicable suggestion. Existing
/// suggestions (e.g. a 旧字体→新字体 one from [`crate::kyuji`]) are left as-is.
pub fn annotate(findings: &mut [Finding]) {
    // Spans already covered by another finding's suggestion — e.g. a 旧字体→新字体
    // fix from the kyuji layer, which flags the *same* one-character span as the
    // moji 外字注記 warning when a char is both 旧字体 and 第3/第4水準. Adding a
    // second, overlapping suggestion there would make `--fix` apply both and
    // corrupt the text, so we leave those spans to the existing suggestion.
    let covered: Vec<Span> = findings
        .iter()
        .filter(|f| f.suggestion.is_some())
        .map(|f| f.span)
        .collect();
    for f in &mut *findings {
        if f.suggestion.is_some() {
            continue;
        }
        let Some(c) = f.codepoint else { continue };
        if covered
            .iter()
            .any(|s| s.start < f.span.end && f.span.start < s.end)
        {
            continue;
        }
        let info = lookup(c);
        // Only suggest when there is a real description to print; otherwise the
        // 注記 would carry empty 「」 and read as unfinished.
        if info.descriptions.is_empty() {
            continue;
        }
        if let Some(chuki) = info.chuki {
            // Only offer the 注記 as a fix when it is itself character-conformant.
            // Some 外字注記辞書 descriptions contain 機種依存 glyphs (e.g. a 「−」
            // composition separator); suggesting such a 注記 would just trade one
            // finding for another, so we skip it rather than offer a bad fix.
            if crate::moji::check(&chuki).is_empty() {
                f.suggestion = Some(Suggestion {
                    label: format!("「{c}」→ {chuki}"),
                    replacement: chuki,
                    span: f.span,
                });
            }
        }
    }
}

/// Build the 外字注記 form for a 第3/第4水準 cell:
/// `※［＃「desc」、第N水準 men-ku-ten］`. Returns `None` for 第1/第2水準
/// characters (which are usable literally and need no 注記).
fn chuki_form(m: MenKuTen, desc: Option<&str>) -> Option<String> {
    let level = match m.level {
        Suijun::Level3 => 3,
        Suijun::Level4 => 4,
        _ => return None,
    };
    let desc = desc.unwrap_or("");
    Some(format!(
        "※［＃「{desc}」、第{level}水準{}-{}-{}］",
        m.men, m.ku, m.ten
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_third_level_char() {
        let info = lookup('\u{4FF1}'); // 俱, 第3水準 1-14-1
        let m = info.men_ku_ten.expect("俱 is in JIS X 0213");
        assert_eq!((m.men, m.ku, m.ten), (1, 14, 1));
        assert!(info.chuki.as_deref().unwrap().contains("第3水準1-14-1"));
    }

    #[test]
    fn jisx0208_char_has_no_chuki() {
        let info = lookup('亜'); // 第1水準 — usable literally
        assert!(info.chuki.is_none());
    }

    #[test]
    fn men_ku_ten_roundtrips_and_search_works() {
        assert_eq!(from_men_ku_ten(1, 14, 1), Some('\u{4FF1}'));
        let descs = gaiji_descriptions('\u{20089}');
        assert!(!descs.is_empty());
        assert!(search(descs[0]).iter().any(|&(_, c)| c == '\u{20089}'));
    }

    use crate::finding::{FindingSource, Origin, Severity, Span};

    fn char_finding(c: Option<char>) -> Finding {
        Finding {
            code: crate::moji::codes::NEEDS_GAIJI_CHUKI,
            severity: Severity::Warning,
            origin: Origin::Character,
            source: FindingSource::Source,
            span: Span { start: 5, end: 9 },
            message: String::new(),
            codepoint: c,
            suggestion: None,
        }
    }

    #[test]
    fn annotate_attaches_chuki_to_a_described_gaiji_char() {
        // U+3094 (ゔ) is 第3水準 1-4-84 with the conformant description
        // 「濁点付き平仮名う」, so the offered 注記 is itself character-clean.
        let mut findings = vec![char_finding(Some('\u{3094}'))];
        annotate(&mut findings);
        let s = findings[0]
            .suggestion
            .as_ref()
            .expect("a described 第3/第4水準 char gets a 外字注記 suggestion");
        assert!(s.replacement.starts_with("※［＃"));
        assert!(s.replacement.contains("第3水準1-4-84"));
        // The suggestion targets the finding's own span (an in-place autofix).
        assert_eq!(s.span, Span { start: 5, end: 9 });
    }

    #[test]
    fn annotate_skips_a_chuki_whose_description_is_non_conformant() {
        // U+20089 is 第4水準, but its 外字注記辞書 description (「尓－小」)
        // contains a 機種依存 separator — suggesting that 注記 would trade one
        // finding for another, so the layer offers nothing rather than a bad fix.
        let mut findings = vec![char_finding(Some('\u{20089}'))];
        annotate(&mut findings);
        assert!(findings[0].suggestion.is_none());
    }

    #[test]
    fn annotate_skips_jisx0208_and_non_char_findings() {
        // 亜 is 第1水準 (chuki None); 'A' and a None codepoint have no 注記.
        let mut findings = vec![
            char_finding(Some('亜')),
            char_finding(Some('A')),
            char_finding(None),
        ];
        annotate(&mut findings);
        assert!(findings.iter().all(|f| f.suggestion.is_none()));
    }

    #[test]
    fn annotate_skips_a_span_already_covered_by_another_finding() {
        // Mimics a char that is both 旧字体 and 第3/第4水準: two findings on the
        // same span, one already carrying a 新字体 suggestion. The gaiji layer
        // must leave the bare one alone, or `--fix` would apply two overlapping
        // replacements and corrupt the text.
        let span = Span { start: 0, end: 3 };
        let kyuji = Finding {
            suggestion: Some(Suggestion {
                replacement: "即".to_owned(),
                span,
                label: "kyuji".to_owned(),
            }),
            span,
            ..char_finding(Some('\u{3094}'))
        };
        let mut bare = char_finding(Some('\u{3094}'));
        bare.span = span;
        let mut findings = vec![kyuji, bare];
        annotate(&mut findings);
        assert!(
            findings[1].suggestion.is_none(),
            "the bare finding on a covered span must not get a gaiji suggestion"
        );
    }

    #[test]
    fn annotate_does_not_overwrite_an_existing_suggestion() {
        let kept = Suggestion {
            replacement: "来".to_owned(),
            span: Span { start: 5, end: 9 },
            label: "kyuji".to_owned(),
        };
        let mut f = char_finding(Some('\u{3094}'));
        f.suggestion = Some(kept);
        let mut findings = vec![f];
        annotate(&mut findings);
        assert_eq!(findings[0].suggestion.as_ref().unwrap().label, "kyuji");
    }
}

//! Conformance corpus — a regression suite of known inputs → expected finding
//! codes.
//!
//! Character-level codes are asserted as a **subset** of what `run_all`
//! produces (robust to a character legitimately tripping more than one layer,
//! and to message-wording changes); clean inputs must produce nothing; the
//! notation and encoding cases assert by origin / code presence.

#![allow(
    clippy::expect_used,
    reason = "test fixtures are required to produce complete reports"
)]

use aozora_proof_core::{FixOperation, Orthography, run_all, run_submission_with_orthography};

/// `(name, input, finding codes that MUST be present)` — empty means "clean".
const CHAR_CASES: &[(&str, &str, &[&str])] = &[
    ("clean", "青空文庫のふつうの文章。", &[]),
    (
        "halfwidth_katakana",
        "\u{FF71}", // ｱ
        &["aozora::proof::character::halfwidth_kana"],
    ),
    (
        "platform_dependent",
        "\u{2460}", // ①
        &["aozora::proof::character::platform_dependent"],
    ),
    (
        "needs_gaiji_chuki",
        "\u{4FF1}", // 俱 (第3水準)
        &["aozora::proof::character::needs_gaiji"],
    ),
    (
        "not_in_jisx0213",
        "\u{1F363}", // 🍣
        &["aozora::proof::character::needs_gaiji"],
    ),
    ("utf8_bom", "\u{FEFF}あ", &["aozora::proof::encoding::bom"]),
    ("bare_lf", "a\nb", &["aozora::proof::encoding::line_ending"]),
    (
        "mixed_line_endings",
        "a\r\nb\nc",
        &["aozora::proof::encoding::line_ending"],
    ),
    ("tab_character", "a\tb", &["aozora::proof::layout::tab"]),
    (
        "form_feed_character",
        "a\u{000C}b",
        &["aozora::proof::layout::form_feed"],
    ),
    (
        "control_character",
        "a\u{007F}b",
        &["aozora::proof::character::control"],
    ),
    (
        "halfwidth_kana_punctuation",
        "\u{FF62}",
        &["aozora::proof::character::halfwidth_kana"],
    ),
];

#[test]
fn char_level_corpus() {
    for (name, input, expected) in CHAR_CASES {
        let report = run_all(input.as_bytes()).expect("conformance input checks");
        let codes: Vec<&str> = report.findings.iter().map(|f| f.code).collect();
        if expected.is_empty() {
            assert!(
                report.findings.is_empty(),
                "[{name}] expected clean, got {codes:?}"
            );
        } else {
            for code in *expected {
                assert!(
                    codes.contains(code),
                    "[{name}] missing {code}; got {codes:?}"
                );
            }
        }
    }
}

#[test]
fn gaiji_layer_suggests_a_chuki_for_a_needs_chuki_char() {
    let report = run_all("\u{3094}".as_bytes()).expect("gaiji input checks");
    let f = report
        .findings
        .iter()
        .find(|f| f.codepoint == Some('\u{3094}'))
        .expect("the 第3/第4水準 char is flagged");
    let s = f
        .fixes
        .first()
        .expect("the gaiji layer attaches a review fix");
    assert!(matches!(s.operation, FixOperation::Text(_)));
    if let FixOperation::Text(edit) = &s.operation {
        assert!(
            edit.replacement.starts_with("※［＃"),
            "expected a 外字注記 form, got {:?}",
            edit.replacement
        );
        assert!(edit.replacement.contains("第3水準1-4-84"));
        assert!(
            run_all(edit.replacement.as_bytes())
                .expect("suggestion checks")
                .findings
                .is_empty()
        );
    }
}

#[test]
fn overlapping_kyuji_and_gaiji_yields_one_clean_suggestion() {
    let report = run_submission_with_orthography("卽".as_bytes(), Orthography::Modern)
        .expect("orthography input checks");
    let suggested: Vec<&str> = report
        .findings
        .iter()
        .flat_map(|finding| &finding.fixes)
        .filter_map(|fix| match &fix.operation {
            FixOperation::Text(edit) => Some(edit.replacement.as_str()),
            FixOperation::RemoveBom
            | FixOperation::NormalizeCrLf
            | FixOperation::EnsureFinalNewline
            | FixOperation::EncodeShiftJis => None,
        })
        .collect();
    assert_eq!(
        suggested,
        vec!["即"],
        "exactly one suggestion (the 新字体 fix), not also a 外字注記"
    );
}

#[test]
fn invalid_encoding_is_reported() {
    assert!(matches!(
        run_all(&[0xFF, 0xFE, 0xFF]),
        Err(aozora_proof_core::CheckError::Decode { .. })
    ));
}

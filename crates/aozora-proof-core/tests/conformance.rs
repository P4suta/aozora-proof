//! Conformance corpus — a regression suite of known inputs → expected finding
//! codes.
//!
//! Character-level codes are asserted as a **subset** of what `run_all`
//! produces (robust to a character legitimately tripping more than one layer,
//! and to message-wording changes); clean inputs must produce nothing; the
//! notation and encoding cases assert by origin / code presence.

use aozora_proof_core::run_all;

/// `(name, input, finding codes that MUST be present)` — empty means "clean".
const CHAR_CASES: &[(&str, &str, &[&str])] = &[
    ("clean", "青空文庫のふつうの文章。", &[]),
    (
        "halfwidth_katakana",
        "\u{FF71}", // ｱ
        &["aozora::char::halfwidth_katakana"],
    ),
    (
        "platform_dependent",
        "\u{2460}", // ①
        &["aozora::char::platform_dependent"],
    ),
    (
        "needs_gaiji_chuki",
        "\u{4FF1}", // 俱 (第3水準)
        &["aozora::char::needs_gaiji_chuki"],
    ),
    (
        "not_in_jisx0213",
        "\u{1F363}", // 🍣
        &["aozora::char::not_in_jisx0213"],
    ),
    ("utf8_bom", "\u{FEFF}あ", &["aozora::char::utf8_bom"]),
    ("bare_lf", "a\nb", &["aozora::char::crlf_expected"]),
    (
        "kyuji",
        "\u{4F86}", // 來 → 来
        &["aozora::kyuji::has_shinji_form"],
    ),
];

#[test]
fn char_level_corpus() {
    for (name, input, expected) in CHAR_CASES {
        let report = run_all(input.as_bytes());
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
    // U+3094 (ゔ) is 第3水準 1-4-84 with a conformant description: run_all flags
    // it as needing a 外字注記, and the gaiji layer attaches the concrete 注記
    // inline as a suggestion (so `check --fix` / the web can offer it).
    let report = run_all("\u{3094}".as_bytes());
    let f = report
        .findings
        .iter()
        .find(|f| f.codepoint == Some('\u{3094}'))
        .expect("the 第3/第4水準 char is flagged");
    let s = f
        .suggestion
        .as_ref()
        .expect("the gaiji layer attaches a 外字注記 suggestion");
    assert!(
        s.replacement.starts_with("※［＃"),
        "expected a 外字注記 form, got {:?}",
        s.replacement
    );
    assert!(s.replacement.contains("第3水準1-4-84"));
    // The suggested 注記 must itself be character-conformant (the fix never
    // trades one finding for another).
    assert!(run_all(s.replacement.as_bytes()).findings.is_empty());
}

#[test]
fn overlapping_kyuji_and_gaiji_yields_one_clean_suggestion() {
    // 卽 (U+537D) is BOTH 旧字体 (→ 即) and 第3水準 1-14-81, so the kyuji and moji
    // layers flag the SAME span. The gaiji layer must not add a second,
    // overlapping suggestion there — otherwise `--fix` applies both and corrupts
    // the text. Exactly one suggestion survives, and it is the 新字体 one.
    let report = run_all("卽".as_bytes());
    let suggested: Vec<&str> = report
        .findings
        .iter()
        .filter_map(|f| f.suggestion.as_ref())
        .map(|s| s.replacement.as_str())
        .collect();
    assert_eq!(
        suggested,
        vec!["即"],
        "exactly one suggestion (the 新字体 fix), not also a 外字注記"
    );
}

#[test]
fn invalid_encoding_is_reported() {
    let report = run_all(&[0xFF, 0xFE, 0xFF]);
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == "aozora::char::invalid_encoding")
    );
}

//! Behavioral cases documented by the independent Checkerkun implementation.

use aozora_proof_core::run_all;

fn codes(text: &str) -> Vec<&'static str> {
    run_all(text.as_bytes())
        .findings
        .into_iter()
        .map(|finding| finding.code)
        .collect()
}

#[test]
fn documented_jis_and_gaiji_examples_agree() {
    assert!(codes("森鴎外").is_empty());
    assert!(codes("森鷗外").contains(&"aozora::char::needs_gaiji_chuki"));
}

#[test]
fn documented_character_failures_agree() {
    assert!(codes("ｴ").contains(&"aozora::char::halfwidth_katakana"));
    assert!(codes("①").contains(&"aozora::char::platform_dependent"));
    assert!(codes("\u{0}").contains(&"aozora::char::control_character"));
}

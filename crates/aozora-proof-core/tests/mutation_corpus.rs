//! Synthetic defects seeded into otherwise ordinary 青空文庫 text.

use aozora_proof_core::run_all;

const CASES: &[(&str, &[u8], &str)] = &[
    (
        "halfwidth katakana",
        include_bytes!("fixtures/mutations/halfwidth-katakana.txt"),
        "aozora::char::halfwidth_katakana",
    ),
    (
        "platform dependent",
        include_bytes!("fixtures/mutations/platform-dependent.txt"),
        "aozora::char::platform_dependent",
    ),
    (
        "third level",
        include_bytes!("fixtures/mutations/third-level.txt"),
        "aozora::char::needs_gaiji_chuki",
    ),
    (
        "tab character",
        include_bytes!("fixtures/mutations/control-character.txt"),
        "aozora::char::tab_character",
    ),
];

#[test]
fn every_seeded_defect_is_detected() {
    for (name, input, expected) in CASES {
        let report = run_all(input);
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == *expected),
            "{name}: missing {expected}"
        );
    }
}

#[test]
fn clean_counterpart_stays_clean() {
    assert!(run_all("青空文庫\r\n".as_bytes()).findings.is_empty());
}

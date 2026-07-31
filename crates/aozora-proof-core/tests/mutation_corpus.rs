//! Synthetic defects seeded into otherwise ordinary 青空文庫 text.

#![allow(
    clippy::expect_used,
    reason = "test fixtures are required to produce complete reports"
)]

use aozora_proof_core::run_all;

const CASES: &[(&str, &[u8], &str)] = &[
    (
        "halfwidth katakana",
        include_bytes!("fixtures/mutations/halfwidth-katakana.txt"),
        "aozora::proof::character::halfwidth_kana",
    ),
    (
        "platform dependent",
        include_bytes!("fixtures/mutations/platform-dependent.txt"),
        "aozora::proof::character::platform_dependent",
    ),
    (
        "third level",
        include_bytes!("fixtures/mutations/third-level.txt"),
        "aozora::proof::character::needs_gaiji",
    ),
    (
        "tab character",
        include_bytes!("fixtures/mutations/control-character.txt"),
        "aozora::proof::layout::tab",
    ),
];

#[test]
fn every_seeded_defect_is_detected() {
    for (name, input, expected) in CASES {
        let report = run_all(input).expect("mutation input checks");
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
    assert!(
        run_all("青空文庫\r\n".as_bytes())
            .expect("clean input checks")
            .findings
            .is_empty()
    );
}

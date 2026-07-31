//! Foundation tests: the aozora→Finding integration spine and wire envelope.

#![allow(
    clippy::expect_used,
    reason = "test fixtures are required to produce complete reports"
)]

use aozora_proof_core::{Origin, run_all, run_notation, serialize_report};

#[test]
fn empty_report_serializes_as_schema_v2() {
    let report = run_all(b"").expect("empty source checks");
    let json = serialize_report(&report).expect("empty report serializes");
    assert!(json.starts_with(r#"{"schemaVersion":2,"tool":"#));
    assert!(json.contains(r#""files":[{"path":"<memory>""#));
}

#[test]
fn notation_layer_runs_without_panic_and_is_tagged_notation() {
    // Whatever diagnostics aozora emits across these inputs, they must come
    // through as Notation-origin findings inside a well-formed envelope.
    for src in [
        "",
        "ふつうの文章。",
        "｜青梅《おうめ》",
        "［＃ここから2字下げ",
        "※［＃「謎の字」、第3水準9-9-9］",
    ] {
        let findings = run_notation(src).expect("notation checks");
        assert!(
            findings
                .iter()
                .all(|f| matches!(f.origin, Origin::Notation)),
            "non-notation origin leaked for input {src:?}"
        );
        assert!(
            findings.iter().all(|finding| !finding.message.is_empty()),
            "missing canonical message for input {src:?}"
        );
    }
}

#[test]
fn notation_spans_use_original_source_coordinates() {
    let source = "\u{feff}前\r\nカフェ〔cafe'〕で待つ";
    let findings = run_notation(source).expect("notation checks");
    let finding = findings
        .iter()
        .find(|finding| finding.code == "aozora::lex::accent_decomposition_applied")
        .expect("accent decomposition diagnostic");
    let start = usize::try_from(finding.span.start).expect("short source");
    let end = usize::try_from(finding.span.end).expect("short source");
    assert_eq!(source.get(start..end), Some("〔cafe'〕"));
}

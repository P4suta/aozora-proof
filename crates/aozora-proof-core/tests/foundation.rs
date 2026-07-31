//! Foundation tests: the aozora→Finding integration spine and wire envelope.

use aozora_proof_core::{Origin, run_notation, serialize_findings};

#[test]
fn empty_findings_serialize_to_empty_envelope() {
    assert_eq!(serialize_findings(&[]), r#"{"schema_version":1,"data":[]}"#);
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
        let findings = run_notation(src);
        assert!(
            findings
                .iter()
                .all(|f| matches!(f.origin, Origin::Notation)),
            "non-notation origin leaked for input {src:?}"
        );
        let json = serialize_findings(&findings);
        assert!(
            json.starts_with(r#"{"schema_version":1,"data":["#),
            "malformed envelope for input {src:?}: {json}"
        );
    }
}

#[test]
fn notation_spans_use_original_source_coordinates() {
    let source = "\u{feff}前\r\nカフェ〔cafe'〕で待つ";
    let findings = run_notation(source);
    let finding = findings
        .iter()
        .find(|finding| finding.code == "aozora::lex::accent_decomposition_applied")
        .expect("accent decomposition diagnostic");
    let start = usize::try_from(finding.span.start).expect("short source");
    let end = usize::try_from(finding.span.end).expect("short source");
    assert_eq!(source.get(start..end), Some("〔cafe'〕"));
}

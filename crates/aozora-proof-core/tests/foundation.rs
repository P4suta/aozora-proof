//! Foundation tests: the aozora→Finding integration spine, the wire
//! envelope, and the sanitized→decoded coordinate map.

use aozora_proof_core::coords::SpanMap;
use aozora_proof_core::{Origin, run_notation, serialize_findings};
use proptest::prelude::*;

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
fn coords_map_is_exact_for_crlf() {
    // "a\r\nb" → sanitized "a\nb". The '\n' at sanitized offset 1 lifts to
    // the decoded '\n' at byte offset 2 (skipping the '\r' at decoded 1).
    let map = SpanMap::build("a\r\nb");
    assert_eq!(map.offset(0), 0, "'a'");
    assert_eq!(map.offset(1), 2, "'\\n' inside the CRLF");
    assert_eq!(map.offset(2), 3, "'b'");
}

#[test]
fn coords_map_is_identity_without_edits() {
    let s = "ふつうの文";
    let map = SpanMap::build(s);
    let len = u32::try_from(s.len()).unwrap();
    for k in 0..=len {
        assert_eq!(map.offset(k), k, "offset({k}) should be identity");
    }
}

// Generate strings from a small alphabet that exercises the sanitize
// transforms (CRLF, lone CR, BOM) alongside ordinary text.
fn sanitize_fodder() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        Just("\r\n".to_owned()),
        Just("\n".to_owned()),
        Just("\r".to_owned()),
        Just("\u{feff}".to_owned()),
        Just("a".to_owned()),
        Just("あ".to_owned()),
        Just("。".to_owned()),
    ];
    prop::collection::vec(piece, 0..24).prop_map(|parts| parts.concat())
}

proptest! {
    /// The map is always in-bounds and monotonically non-decreasing — the
    /// invariants every downstream caret / SARIF region relies on.
    #[test]
    fn coords_map_is_monotonic_and_in_bounds(s in sanitize_fodder()) {
        let map = SpanMap::build(&s);
        let len = u32::try_from(s.len()).unwrap();
        let mut prev = 0u32;
        for k in 0..=len {
            let o = map.offset(k);
            prop_assert!(o <= len, "offset({k})={o} exceeds len {len}");
            prop_assert!(o >= prev, "offset not monotonic at {k}: {o} < {prev}");
            prev = o;
        }
    }
}

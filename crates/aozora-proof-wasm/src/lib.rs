//! WASM façade over [`aozora_proof_core`] for the static web app.
//!
//! Compiles to a `wasm32-unknown-unknown` artifact via
//! `wasm-pack build --target web --release crates/aozora-proof-wasm`, exposing
//! the proofreading check and the 外字 search to JS / TypeScript.
//!
//! The `#[wasm_bindgen]` exports are gated on `cfg(target_arch = "wasm32")`, so
//! host builds of the workspace compile them as plain functions and skip the
//! wasm-bindgen dependency entirely.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Run submission checks over UTF-8 text with direction-neutral orthography.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = checkJson))]
#[must_use]
pub fn check_json(text: &str) -> String {
    let report = aozora_proof_core::run_submission_with_orthography(
        text.as_bytes(),
        aozora_proof_core::Orthography::Mixed,
    );
    aozora_proof_core::serialize_report(&report)
}

/// Search the 外字注記辞書 for descriptions containing `query`; returns a JSON
/// object `{ "matches": [ { "description", "char", "codepoint" }, … ] }`.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = gaijiSearchJson))]
#[must_use]
pub fn gaiji_search_json(query: &str) -> String {
    let matches: Vec<serde_json::Value> = aozora_proof_core::gaiji_dict::search(query)
        .iter()
        .map(|&(desc, c)| {
            serde_json::json!({
                "description": desc,
                "char": c.to_string(),
                "codepoint": format!("U+{:04X}", u32::from(c)),
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({ "matches": matches }))
        .unwrap_or_else(|_| String::from(r#"{"matches":[]}"#))
}

/// Map every documented finding code to its Japanese title.
///
/// The web app shows this readable category instead of the raw internal code.
/// Codes without a `RuleDoc` (e.g. notation `aozora::lex::*`) are absent.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = ruleTitlesJson))]
#[must_use]
pub fn rule_titles_json() -> String {
    let map: serde_json::Map<String, serde_json::Value> = aozora_proof_core::all_rules()
        .iter()
        .map(|rule| (rule.code.to_owned(), serde_json::Value::from(rule.title_ja)))
        .collect();
    serde_json::to_string(&map).unwrap_or_else(|_| String::from("{}"))
}

/// Return the bilingual rule catalog shared with the native CLI.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = ruleCatalogJson))]
#[must_use]
pub fn rule_catalog_json() -> String {
    let rules: Vec<serde_json::Value> = aozora_proof_core::all_rules()
        .iter()
        .map(|rule| {
            serde_json::json!({
                "code": rule.code,
                "category": rule.category.as_wire_str(),
                "severity": rule.default_severity.as_wire_str(),
                "title": { "en": rule.title, "ja": rule.title_ja },
                "rationale": { "en": rule.rationale, "ja": rule.rationale_ja },
                "detection": rule.detection.as_wire_str(),
                "fix": rule.fix.map(aozora_proof_core::FixApplicability::as_wire_str),
                "authorityUrl": rule.authority_url,
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({ "rules": rules }))
        .unwrap_or_else(|_| String::from(r#"{"rules":[]}"#))
}

/// The wire-format schema version (matches `aozora_proof_core::SCHEMA_VERSION`).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = schemaVersion))]
#[must_use]
#[allow(
    clippy::missing_const_for_fn,
    reason = "#[wasm_bindgen] requires a non-const fn on wasm32"
)]
pub fn schema_version() -> u32 {
    aozora_proof_core::SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_json_emits_envelope() {
        let json = check_json("\u{2460}"); // ①
        assert!(json.starts_with(r#"{"schemaVersion":2,"tool":"#));
        assert!(json.contains("platform_dependent"));
    }

    #[test]
    fn gaiji_search_json_emits_matches() {
        let json = gaiji_search_json("尓－小");
        assert!(json.contains("\"matches\""));
        assert!(json.contains("U+20089"));
    }

    #[test]
    fn rule_titles_json_maps_code_to_title() {
        let json = rule_titles_json();
        let map: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            map.get("aozora::proof::character::platform_dependent")
                .and_then(serde_json::Value::as_str),
            Some("機種依存文字")
        );
        // Notation findings have no RuleDoc, so their codes are absent.
        assert!(map.get("aozora::lex::unterminated_ruby").is_none());
    }

    #[test]
    fn catalog_is_bilingual() {
        let json = rule_catalog_json();
        assert!(json.contains("\"en\""));
        assert!(json.contains("\"ja\""));
    }

    #[test]
    fn schema_version_is_two() {
        assert_eq!(schema_version(), 2);
    }
}

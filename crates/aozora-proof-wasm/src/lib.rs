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

/// Failure surfaced to JavaScript as an exception.
#[derive(Debug, thiserror::Error)]
pub enum WasmError {
    /// The checker could not produce a complete report.
    #[error("proofreading failed: {source}")]
    Check {
        /// Checked pipeline failure.
        #[source]
        source: aozora_proof_core::CheckError,
    },
    /// A JSON response could not be serialized.
    #[error("JSON serialization failed: {source}")]
    Serialize {
        /// Serializer failure.
        #[source]
        source: serde_json::Error,
    },
}

#[cfg(target_arch = "wasm32")]
impl From<WasmError> for JsValue {
    fn from(error: WasmError) -> Self {
        Self::from_str(&error.to_string())
    }
}

/// Run submission checks over UTF-8 text with direction-neutral orthography.
///
/// # Errors
///
/// Returns a JavaScript exception when checking or serialization fails.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = checkJson))]
pub fn check_json(text: &str) -> Result<String, WasmError> {
    check_bytes_json(text.as_bytes())
}

fn check_bytes_json(raw: &[u8]) -> Result<String, WasmError> {
    let report = aozora_proof_core::run_submission_with_orthography(
        raw,
        aozora_proof_core::Orthography::Mixed,
    )
    .map_err(|source| WasmError::Check { source })?;
    aozora_proof_core::serialize_report(&report).map_err(|source| WasmError::Serialize { source })
}

/// Search the 外字注記辞書 for descriptions containing `query`; returns a JSON
/// object `{ "matches": [ { "description", "char", "codepoint" }, … ] }`.
///
/// # Errors
///
/// Returns a JavaScript exception when serialization fails.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = gaijiSearchJson))]
pub fn gaiji_search_json(query: &str) -> Result<String, WasmError> {
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
        .map_err(|source| WasmError::Serialize { source })
}

/// Map every documented finding code to its Japanese title.
///
/// The web app shows this readable category instead of the raw internal code.
/// Codes without a `RuleDoc` (e.g. notation `aozora::lex::*`) are absent.
///
/// # Errors
///
/// Returns a JavaScript exception when serialization fails.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = ruleTitlesJson))]
pub fn rule_titles_json() -> Result<String, WasmError> {
    let map: serde_json::Map<String, serde_json::Value> = aozora_proof_core::all_rules()
        .iter()
        .map(|rule| (rule.code.to_owned(), serde_json::Value::from(rule.title_ja)))
        .collect();
    serde_json::to_string(&map).map_err(|source| WasmError::Serialize { source })
}

/// Return the bilingual rule catalog shared with the native CLI.
///
/// # Errors
///
/// Returns a JavaScript exception when serialization fails.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = ruleCatalogJson))]
pub fn rule_catalog_json() -> Result<String, WasmError> {
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
        .map_err(|source| WasmError::Serialize { source })
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
        let json = check_json("\u{2460}").expect("valid WASM report");
        assert!(json.starts_with(r#"{"schemaVersion":2,"tool":"#));
        assert!(json.contains("platform_dependent"));
    }

    #[test]
    fn gaiji_search_json_emits_matches() {
        let json = gaiji_search_json("尓－小").expect("serializable matches");
        assert!(json.contains("\"matches\""));
        assert!(json.contains("U+20089"));
    }

    #[test]
    fn rule_titles_json_maps_code_to_title() {
        let json = rule_titles_json().expect("serializable titles");
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
        let json = rule_catalog_json().expect("serializable catalog");
        assert!(json.contains("\"en\""));
        assert!(json.contains("\"ja\""));
    }

    #[test]
    fn schema_version_is_two() {
        assert_eq!(schema_version(), 2);
    }

    #[test]
    fn successful_json_matches_the_core_serializer() {
        let text = "ｱ\n";
        let report = aozora_proof_core::run_submission_with_orthography(
            text.as_bytes(),
            aozora_proof_core::Orthography::Mixed,
        )
        .expect("valid report");
        let expected = aozora_proof_core::serialize_report(&report).expect("serializable report");
        assert_eq!(check_json(text).expect("WASM report"), expected);
    }

    #[test]
    fn checker_failure_is_an_error_instead_of_empty_json() {
        assert!(matches!(
            check_bytes_json(&[0xFF, 0xFF, 0xFF]),
            Err(WasmError::Check { .. })
        ));
    }
}

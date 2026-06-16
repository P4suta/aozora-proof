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

/// Run the full proofreading pipeline over `text` (UTF-8) and return the
/// findings as the `{ "schema_version", "data" }` JSON envelope.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = checkJson))]
#[must_use]
pub fn check_json(text: &str) -> String {
    let report = aozora_proof_core::run_all(text.as_bytes());
    aozora_proof_core::serialize_findings(&report.findings)
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

/// The wire-format schema version (matches `aozora_proof_core::SCHEMA_VERSION`).
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = schemaVersion))]
#[must_use]
pub fn schema_version() -> u32 {
    aozora_proof_core::SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_json_emits_envelope() {
        let json = check_json("\u{2460}"); // ①
        assert!(json.starts_with(r#"{"schema_version":1,"data":["#));
        assert!(json.contains("platform_dependent"));
    }

    #[test]
    fn gaiji_search_json_emits_matches() {
        let json = gaiji_search_json("尓－小");
        assert!(json.contains("\"matches\""));
        assert!(json.contains("U+20089"));
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(schema_version(), 1);
    }
}

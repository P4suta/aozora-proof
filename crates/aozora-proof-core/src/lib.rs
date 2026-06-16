//! `aozora-proof-core` — the character-level proofreading engine for
//! 青空文庫 (Aozora Bunko) text.
//!
//! The sibling [`aozora`] parser already covers the **notation level**
//! (ruby, bouten, 外字 resolution, bracket pairing, structured
//! diagnostics). This crate adds the **character level** — JIS X 0208
//! conformance, 機種依存文字, 旧字体↔新字体, half/full-width, and
//! file-structure checks — and merges both into one unified [`Report`].
//!
//! The engine is pure: it takes `&str` / `&[u8]` and returns
//! [`Finding`]s. It performs no I/O, forbids `unsafe`, and is WASM-clean,
//! so the same logic drives the CLI and the static web app.
//!
//! ```
//! use aozora_proof_core::{run_notation, serialize_findings};
//!
//! let findings = run_notation("｜青梅《おうめ》");
//! let json = serialize_findings(&findings); // { "schema_version": 1, "data": [ … ] }
//! assert!(json.starts_with("{\"schema_version\":1"));
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::indexing_slicing,
        reason = "tests index results of asserted length"
    )
)]

pub mod coords;
pub mod finding;
pub mod pipeline;
pub mod rules;

pub mod gaiji_dict;
pub mod kyuji;
pub mod moji;

pub use finding::{
    Finding, FindingSource, Origin, SCHEMA_VERSION, Severity, Span, Suggestion, serialize_findings,
};
pub use pipeline::{Report, run_all, run_notation};
pub use rules::{RuleDoc, all_rules, explain};

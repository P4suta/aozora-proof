//! Pure proofreading engine shared by the CLI and WASM frontends.
//!
//! ```
//! use aozora_proof_core::{
//!     Orthography, run_submission_with_orthography, serialize_report,
//! };
//! # fn main() -> Result<(), String> {
//!
//! let report =
//!     run_submission_with_orthography("青空\r\n".as_bytes(), Orthography::Mixed)
//!         .map_err(|error| error.to_string())?;
//! let json = serialize_report(&report).map_err(|error| error.to_string())?;
//! assert!(json.starts_with(r#"{"schemaVersion":2"#));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![cfg_attr(
    test,
    allow(
        clippy::indexing_slicing,
        reason = "tests index collections after asserting their shape"
    )
)]

mod error;
pub mod finding;
pub mod fix;
pub mod gaiji_dict;
pub mod kyuji;
pub mod moji;
pub mod orthography;
pub mod pipeline;
pub mod review;
pub mod rules;
pub mod submission;
pub mod wire;

pub use error::CheckError;
pub use finding::{
    DetectionClass, Finding, FindingDetails, FindingSource, FixAlternative, FixApplicability,
    FixOperation, Origin, Position, RuleCategory, SCHEMA_VERSION, Severity, Span, TextEdit,
    position,
};
pub use fix::{FixError, SafeFixResult, apply_safe, apply_text_edits};
pub use orthography::Orthography;
pub use pipeline::{
    Report, run_all, run_notation, run_submission, run_submission_with_orthography,
};
pub use rules::{OfficialItem, RuleDoc, all_rules, explain, official_items};
pub use wire::{ReportFile, serialize_report, serialize_reports, serialize_reports_to_writer};

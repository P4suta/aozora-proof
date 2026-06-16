//! Minimal end-to-end preview: parse the text given as the first CLI
//! argument (or a built-in sample) and print the findings envelope. A
//! stand-in for `aozora-proof-cli` until that crate lands.
//!
//! ```text
//! cargo run --example demo -- '※［＃「謎の字」、第3水準9-9-9］'
//! ```

fn main() {
    let text = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "※［＃「謎の字」、第3水準9-9-9］".to_owned());
    let report = aozora_proof_core::run_all(text.as_bytes());
    println!(
        "{}",
        aozora_proof_core::serialize_findings(&report.findings)
    );
}

#![no_main]

use aozora_proof_core::{
    Orthography, run_submission_with_orthography, serialize_report,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let report = run_submission_with_orthography(data, Orthography::Mixed);
    let _ = serialize_report(&report);
});

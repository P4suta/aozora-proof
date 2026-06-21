#![no_main]

//! Fuzz the whole pipeline: `run_all` decodes arbitrary bytes (UTF-8 or
//! Shift_JIS) and runs every layer. It must never panic on any input.

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = aozora_proof_core::run_all(data);
});

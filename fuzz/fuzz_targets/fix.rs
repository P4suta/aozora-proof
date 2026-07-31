#![no_main]

use aozora_proof_core::{Orthography, apply_safe};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    match std::hint::black_box(apply_safe(data, Orthography::Mixed)) {
        Ok(_) | Err(_) => {}
    }
});

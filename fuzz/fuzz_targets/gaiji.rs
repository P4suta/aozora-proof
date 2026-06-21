#![no_main]

//! Fuzz the gaiji lookup surface: reverse 面区点 lookup, description search,
//! and per-character lookup must all be panic-free on arbitrary input.

use libfuzzer_sys::fuzz_target;

use aozora_proof_core::gaiji_dict;

fuzz_target!(|data: &[u8]| {
    if let [men, ku, ten, ..] = data {
        let _ = gaiji_dict::from_men_ku_ten(*men, *ku, *ten);
    }
    let text = String::from_utf8_lossy(data);
    let _ = gaiji_dict::search(&text);
    for c in text.chars().take(64) {
        let _ = gaiji_dict::lookup(c);
    }
});

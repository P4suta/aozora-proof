# aozora-proof-core

Private, pure, WASM-compatible engine behind `aozora-proof`.

It owns the bilingual Rule Catalog, official-requirement coverage,
automatic/review/manual classification, explicit orthography policy,
structured safe/review fixes, fixed-point safe application, and deterministic
schema-v2 serialization. The `aozora` parser remains the authority for
notation parsing and its `aozora::lex::*` diagnostics.

This crate is not a supported Rust API and is not published to crates.io. See
the [architecture guide](../../ARCHITECTURE.md).

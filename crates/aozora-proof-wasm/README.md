# aozora-proof-wasm

The WASM façade over [`aozora-proof-core`](../aozora-proof-core), for the static
web app.

Build the browser package:

```console
$ wasm-pack build --target web --release crates/aozora-proof-wasm
# → crates/aozora-proof-wasm/pkg/  (ES module + .wasm)
```

Exports: `checkJson(text)` (the `{schema_version,data}` findings envelope),
`gaijiSearchJson(query)`, and `schemaVersion()`. On non-wasm targets these
compile as plain Rust functions (the wasm-bindgen dependency is wasm32-only),
so the crate stays part of the host workspace build.

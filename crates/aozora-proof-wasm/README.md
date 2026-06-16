# aozora-proof-wasm

The WASM façade over [`aozora-proof-core`](../aozora-proof-core), for the static
web app.

Build the browser package (writes to `web/pkg/`, where the `web/` app loads it):

```console
$ just wasm
# equivalently:
$ wasm-pack build --target web --release crates/aozora-proof-wasm --out-dir ../../web/pkg
```

Exports: `checkJson(text)` (the `{schema_version,data}` findings envelope),
`gaijiSearchJson(query)`, and `schemaVersion()`. On non-wasm targets these
compile as plain Rust functions (the wasm-bindgen dependency is wasm32-only),
so the crate stays part of the host workspace build.

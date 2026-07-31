# aozora-proof-wasm

Private WebAssembly façade over `aozora-proof-core` for the static web app.

`checkJson(text)` returns the same schema-v2 report as the CLI under the
non-directional `mixed` policy. `ruleTitlesJson()` and `ruleCatalogJson()`
expose localized catalog presentation, while `gaijiSearchJson(query)` and
`schemaVersion()` provide the existing reference APIs.

```console
$ just wasm
```

The generated package is written to `web/src/lib/pkg`. This crate is not
published to crates.io.

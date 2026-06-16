# Architecture

`aozora-proof` proofreads **青空文庫記法 (Aozora Bunko notation)** text. Its one
load-bearing idea is a split of responsibilities:

- **Notation level** — ruby, bouten, 外字 resolution, bracket pairing, and the
  structured diagnostics that come with them — belongs to the sibling
  [`aozora`](https://github.com/P4suta/aozora) parser. We *consume* it; we do
  not reimplement it.
- **Character level** — whether each character may appear literally in
  conformant text (JIS X 0208 conformance, 機種依存文字, half/full-width,
  旧字体↔新字体) plus file-structure checks (BOM, line endings, encoding) — is
  what this repository owns.

`run_all` runs both and merges them into one unified report.

## Crates

```
aozora-proof-cli  ──┐
                    ├──> aozora-proof-core ──> aozora-proof-data
aozora-proof-wasm ──┘
```

`cli` and `wasm` are façades over `core`; `core` bakes in `data`'s tables.

| crate | role | boundary |
|---|---|---|
| `aozora-proof-core` | the engine: `&[u8]` / `&str` → findings | pure, `forbid(unsafe)`, **WASM-clean** (no I/O, no host-only deps) |
| `aozora-proof-data` | JIS 水準 / 機種依存文字 / 旧字体 / gaiji tables | baked at build time from vendored sources via `build.rs` |
| `aozora-proof-cli`  | argument parsing, file I/O, output formatting | the only crate that touches the filesystem |
| `aozora-proof-wasm` | `checkJson` / `gaijiSearchJson` / `schemaVersion` | wasm-bindgen exports gated on `cfg(target_arch = "wasm32")`; plain functions on host builds |

The core stays pure so the *same* engine drives the CLI, the in-browser web app,
and (in time) an editor/LSP server.

## The pipeline

`core::run_all(raw: &[u8]) -> Report` is the spine
([`crates/aozora-proof-core/src/pipeline.rs`](crates/aozora-proof-core/src/pipeline.rs)):

1. **File-structure checks** on the raw bytes — BOM, CRLF vs LF, encoding.
2. **Decode** to UTF-8 (UTF-8, falling back to Shift_JIS).
3. **Notation layer** — hand the decoded text to the `aozora` parser and lift
   each diagnostic into a unified `Finding`.
4. **Character layers** — `moji` (conformance) and `kyuji` (旧字体↔新字体) over
   the decoded text. (`gaiji_dict` powers the reverse-lookup `gaiji`
   subcommand; gaiji-as-a-check is a later milestone.)

Everything is merged into one `Report { findings, decoded }`, sorted by span, in
a single **decoded** coordinate frame.

### Coordinate frames

Three byte-offset frames coexist, and every finding is reported in the
**decoded** one so character and notation findings line up:

- **raw** — original file bytes (BOM, CRLF, original encoding).
- **decoded** — the UTF-8 string the character layers index into.
- **sanitized** — the parser's internal view (BOM stripped, CRLF→LF, accents
  decomposed).

[`coords::SpanMap`](crates/aozora-proof-core/src/coords.rs) lifts the parser's
sanitized spans back into the decoded frame so they merge cleanly with character
findings.

## The wire contract

Output is a stable JSON envelope owned by this repo, independent of the parser's:

```json
{ "schema_version": 1, "data": [ /* findings */ ] }
```

`SCHEMA_VERSION` lives in
[`finding.rs`](crates/aozora-proof-core/src/finding.rs) and is the seam shared by
core ↔ cli ↔ wasm ↔ the web app. Each `Finding` carries a stable string `code`
(e.g. `aozora::char::platform_dependent`), a `severity`, a decoded `span`, a
Japanese `message`, and an optional `suggestion` (e.g. a 旧字体→新字体 rewrite).
The CLI renders this envelope as human / JSON / short / SARIF; the WASM façade
hands it to the web app verbatim.

## Data provenance

`aozora-proof-data` bakes lookup tables at build time from vendored sources:

- **JIS X 0213** mapping (水準 classification, 面区点) — `jisx0213-2004-std.txt`.
- **常用漢字表** 旧字体↔新字体 pairs — `joyo-kyujitai.tsv`.
- **外字注記辞書** descriptions for gaiji search — `aozora-gaiji-chuki.tsv`.

Each carries its own upstream license; see [`NOTICE`](NOTICE).

## Where to add a check

Character checks live in `core` (`moji`, `kyuji`) and emit `Finding`s with a new
stable `code`. Notation-level behavior belongs **upstream** in `aozora`, not
here. Keep `core` free of I/O and `unsafe` so it stays WASM-clean.

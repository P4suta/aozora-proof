# Changelog

All notable changes to aozora-proof are recorded in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-07-31

### Changed

- Replaced the character-checker command surface with submission-oriented
  `check`, safe `fix`, full-screen `review`, and catalog/reference commands.
- Made `modern`, `traditional`, or `mixed` orthography policy mandatory for
  document commands, with flag/environment/project/user precedence.
- Replaced machine schema v1 with deterministic schema v2 file reports and
  canonical English JSON, short, and SARIF output.
- Changed the GitHub Action from a source build to checksum and provenance
  verification of a selected GitHub Release binary.

### Added

- A bilingual Rule Catalog that classifies every represented official
  checklist/manual item as automatic, review, or manual and keeps manual work
  visible in reports and the TUI.
- Typed `Safe` and `Review` fix alternatives, overlap rejection, fixed-point
  application, lossless Shift_JIS validation, concurrent-change refusal, and
  atomic permission-preserving file writes.
- Recursive deterministic `.txt` discovery with ignore, hidden-path, symlink,
  include/exclude, and explicit-file semantics.
- XDG/platform configuration, per-path overrides, unknown-key rejection, and
  did-you-mean diagnostics for rule codes.
- Linux musl, macOS, and Windows release archives with generated manuals and
  completions, SHA-256 files, SPDX SBOM, and provenance attestations.
- Character-conformance engine (`aozora-proof-core`): JIS X 0208 水準
  classification, 機種依存文字 detection, half-width katakana, and
  file-structure checks (BOM, line endings, encoding) — merged with the
  `aozora` parser's notation diagnostics into one sorted report, with a JSON
  wire format that is a superset of the parser's diagnostic shape.
- `aozora-proof-data`: a `char → JIS 水準` classifier and 機種依存文字
  predicate, baked at build time from the Project X0213 mapping table.
- `aozora-proof` CLI output with human / JSON / short / **SARIF** formats,
  stdin/file/directory input, `--fail-on`, and a 0 / 1 / 2 / 3 exit contract.
- **Old-/new-form (旧字体↔新字体) detection** (`aozora-proof-data` + the `kyuji`
  layer): flags 旧字体 / 異体字 characters that have a 新字体 counterpart (derived
  from the 常用漢字表) with directional review alternatives.
- **Gaiji (外字) lookup** (`aozora-proof-data` + the `gaiji_dict` module + the
  `aozora-proof gaiji` subcommand): character ⇔ JIS 面区点 ⇔ Unicode and
  description search over the CC0 外字注記辞書, with a suggested 外字注記 form.
- A composite **GitHub Action** (`action/`) that runs the checks and uploads
  SARIF to the Security tab, and a **pre-commit** hook (`.pre-commit-hooks.yaml`)
  for downstream `.txt` repositories.
- A **WebAssembly package** (`aozora-proof-wasm`) and a **static web app**
  (`web/`) that run the checks entirely in the browser (paste text → findings,
  plus 外字 search), deployed to GitHub Pages alongside the rustdoc API at `/api/`.

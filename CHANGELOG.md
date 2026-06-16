# Changelog

All notable changes to aozora-proof are recorded in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Character-conformance engine (`aozora-proof-core`): JIS X 0208 水準
  classification, 機種依存文字 detection, half-width katakana, and
  file-structure checks (BOM, line endings, encoding) — merged with the
  `aozora` parser's notation diagnostics into one sorted report, with a JSON
  wire format that is a superset of the parser's diagnostic shape.
- `aozora-proof-data`: a `char → JIS 水準` classifier and 機種依存文字
  predicate, baked at build time from the Project X0213 mapping table.
- `aozora-proof` CLI: `check` with human / JSON / short / **SARIF** output, stdin
  and multi-file input, `--strict` / `--fail-on`, and a 0 / 1 / 2 / 3 exit-code
  contract for CI.
- A composite **GitHub Action** (`action/`) that runs the checks and uploads
  SARIF to the Security tab, and a **pre-commit** hook (`.pre-commit-hooks.yaml`)
  for downstream `.txt` repositories.

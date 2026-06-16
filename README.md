# aozora-proof

<p align="center">
  <a href="https://github.com/P4suta/aozora-proof/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/P4suta/aozora-proof/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/P4suta/aozora-proof/releases/latest"><img alt="latest release" src="https://img.shields.io/github/v/release/P4suta/aozora-proof?display_name=tag&sort=semver"></a>
  <a href="./LICENSE-APACHE"><img alt="license" src="https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue"></a>
  <a href="./rust-toolchain.toml"><img alt="msrv" src="https://img.shields.io/badge/rust-1.95%2B-orange"></a>
</p>

A modern, FOSS proofreading toolkit for **青空文庫記法 (Aozora Bunko notation)**
text — built to run locally, in **CI**, and in the browser as a static web app.

`aozora-proof` checks the **character level** of a manuscript — the layer the
[`aozora`](https://github.com/P4suta/aozora) parser deliberately leaves alone —
and folds in the parser's notation diagnostics into one unified report:

- **Character conformance** — flags characters that may not appear literally in
  conformant text (outside JIS X 0208 / 機種依存文字 / half-width katakana) and
  must instead be written as 外字注記; plus file-structure checks (BOM, line
  endings, encoding).
- **Old-/new-form kanji (旧字体↔新字体)** — detects kanji that have an
  old/new-form counterpart and suggests the alternate for the editor to confirm.
- **Gaiji (外字) lookup** — 注記 ⇔ character ⇔ JIS 面区点 ⇔ Unicode, both ways.

The notation level (ruby, bouten, 外字 resolution, bracket pairing, diagnostics)
is handled by the `aozora` parser, which `aozora-proof` consumes rather than
reimplements.

## Use

```console
$ aozora-proof check seihon.txt
$ cat seihon.txt | aozora-proof check -
$ aozora-proof check --format json *.txt          # machine-readable, for CI
$ aozora-proof check --fail-on warning chapter*.txt
```

Exit codes: `0` clean · `1` findings (`--strict`, or at/above `--fail-on`) ·
`2` usage / IO error · `3` internal-source finding (a tool bug).

## CI / pre-commit

GitHub Action — runs the checks and uploads findings to the Security tab as SARIF:

```yaml
# .github/workflows/aozora-proof.yml
permissions:
  contents: read
  security-events: write
jobs:
  proof:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: P4suta/aozora-proof/action@main
        with:
          files: "**/*.txt"
          fail-on: error
```

pre-commit ([pre-commit.com](https://pre-commit.com)):

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/P4suta/aozora-proof
    rev: main
    hooks:
      - id: aozora-proof
```

## Workspace

| crate | role |
|---|---|
| `aozora-proof-core` | the engine — pure, `forbid(unsafe)`, WASM-clean; `&str` / `&[u8]` → findings |
| `aozora-proof-data` | character-classification tables (JIS 水準, 機種依存文字, 旧字体, gaiji), baked at build time |
| `aozora-proof-cli`  | the `aozora-proof` binary |
| `aozora-proof-wasm` | wasm-bindgen façade powering the in-browser web app (`web/`) |

A static **web app** (`web/`) runs the checks in the browser — paste text to see
findings plus 外字 search — published to
[GitHub Pages](https://p4suta.github.io/aozora-proof/).

## Develop

`./bootstrap.sh` provisions the toolchain and dev tools; `just --list` shows
every task. See [CONTRIBUTING](CONTRIBUTING.md) and [ARCHITECTURE](ARCHITECTURE.md).

## License

Apache-2.0 OR MIT, at your option. Vendored character data carries its own
upstream licenses; see [`NOTICE`](NOTICE).

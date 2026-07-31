# aozora-proof

`aozora-proof` is a submission-quality proofreader for
[青空文庫](https://www.aozora.gr.jp/) text. Version 0.2 combines complete
submission checks, deliberately narrow safe fixes, and interactive review in
one deterministic CLI.

The tool distinguishes three kinds of requirement:

- **automatic** checks can be decided from the file, including Shift_JIS,
  CRLF, BOM, character repertoire, control characters, external-character
  annotations, notation structure, submission wrappers, and the final newline;
- **review** findings present evidence and alternatives without deciding
  orthography, OCR ambiguities, spacing, ruby grouping, `ケ`/`ヶ`, gaiji, or
  bibliographical intent;
- **manual** checks such as comparison with the base edition, semantic layout,
  and rights decisions stay visible and are never reported as automatically
  confirmed.

The [official checklist](https://www.aozora.gr.jp/KOSAKU/textfile_checklist/)
and [proofreading manual](https://www.aozora.gr.jp/aozora-manual/index-proofreading.html)
are mapped to the catalog exercised by `aozora-proof rules`.

## Install

Download the archive for your platform from
[GitHub Releases](https://github.com/P4suta/aozora-proof/releases). Each archive
includes the binary, manual page, shell completions, licenses, and its SHA-256
file. Releases also publish an aggregate checksum, SBOM, and GitHub artifact
attestation.

The Rust workspace crates are private implementation modules and are not
published to crates.io.

## Use

Every document command requires an explicit character-form policy:

- `modern` reviews traditional forms as modern-form candidates;
- `traditional` reviews modern forms with all known traditional candidates;
- `mixed` performs no directional orthography review.

```console
$ aozora-proof check --orthography modern manuscript.txt
$ aozora-proof check --orthography mixed src/ chapter.txt
$ cat manuscript.txt | aozora-proof check --orthography mixed --format json -
$ aozora-proof fix --orthography mixed --dry-run manuscript.txt
$ aozora-proof fix --orthography mixed manuscript.txt
$ aozora-proof review --orthography traditional manuscript.txt
$ aozora-proof explain aozora::proof::encoding::line_ending
$ aozora-proof gaiji lookup U+4FF1
$ aozora-proof gaiji search 葛
$ aozora-proof rules
$ aozora-proof config show manuscript.txt
```

`check` is always read-only. `fix` applies only `Safe` edits, validates all
edits in memory, requires a lossless Shift_JIS result, and atomically replaces
a file only if it has not changed since reading. `fix -n` prints a unified
diff. With stdin, `fix` writes the corrected Shift_JIS document to stdout.

`review` requires a terminal and stages choices in memory. `y/n/q/a/d/g/j/k`,
`/`, `p`, and `?` follow the familiar patch-review model; `Ctrl-S` shows the
final diff, and only a second confirmation writes. `Esc` and `Ctrl-C` exit
without changes.

## Configuration

Resolution order is command flag, `AOZORA_PROOF_*` environment variable,
nearest `.aozora-proof.toml`, platform user configuration, then default. On
Unix the user file is
`$XDG_CONFIG_HOME/aozora-proof/config.toml`, falling back to
`$HOME/.config/aozora-proof/config.toml`.

```toml
orthography = "mixed"
fail-on = "error"
format = "auto"
color = "auto"
lang = "en"
include = ["**/*.txt"]
exclude = ["vendor/**"]
respect-ignore = true
autofix = true

[rules]
"aozora::proof::layout::spacing" = "off"

[[overrides]]
path = "classics/**"
orthography = "traditional"
```

`aozora-proof init` creates a project configuration interactively.
`aozora-proof config schema` prints its JSON Schema. Unknown keys and rule
codes are usage errors; rule codes include a closest-match suggestion.

Directory traversal is recursive for `.txt` files, skips hidden paths and
symlinked directories, and respects `.gitignore`, `.ignore`, and
`.aozora-proofignore`. An explicitly named file is checked even when ignored.

## Output and exits

`--format auto` selects human output on a terminal and schema-v2 JSON when
piped. Explicit formats are `human`, `json`, `short`, and `sarif`. JSON, short,
and SARIF use canonical English and deterministic ordering; `--lang en|ja`
changes human presentation only.

Exit codes are `0` for success, `1` for findings at or above `--fail-on`, `2`
for usage/input/configuration/write failures, and `3` for an internal
invariant finding. SIGINT remains 130. A closed output pipe exits successfully.

Machine JSON begins with `schemaVersion`, `tool`, `summary`, and `files`. Each
file records encoding, line endings, conformance, review state, and findings
with decoded UTF-8 byte spans, one-based Unicode code-point positions,
canonical messages, authority URLs, structured data, and typed alternatives.

## CI and pre-commit

The composite action downloads the requested release archive, checks its
SHA-256 digest, verifies its GitHub artifact attestation, runs the CLI, and can
upload SARIF:

```yaml
permissions:
  contents: read
  security-events: write
  attestations: read

steps:
  - uses: actions/checkout@v6
  - uses: P4suta/aozora-proof/action@v0.2.0
    with:
      files: "**/*.txt"
      orthography: mixed
      fail-on: error
      version: "0.2.0"
```

The repository also exposes an `aozora-proof` pre-commit hook with the
non-directional `mixed` policy. It uses the release binary already on `PATH`
and does not build the private crates.

## Develop

`./bootstrap.sh` provisions the pinned environment, `just --list` documents the
development commands, and `just ci` is the pre-push gate. See
[CONTRIBUTING](CONTRIBUTING.md), [architecture](ARCHITECTURE.md), and
[ADR 0005](docs/adr/0005-submission-proofreading-cli.md).

The static [web app](https://p4suta.github.io/aozora-proof/) uses the same
schema-v2 catalog through WebAssembly. Its editor workflow is intentionally
unchanged in 0.2.

## License

Apache-2.0 OR MIT, at your option. Vendored character data carries its own
upstream licenses; see [NOTICE](NOTICE).

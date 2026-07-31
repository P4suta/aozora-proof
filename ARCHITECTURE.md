# Architecture

`aozora-proof` adds submission policy around the
[`aozora`](https://github.com/P4suta/aozora) parser. The parser remains
responsible for notation syntax and original-source diagnostics; this
repository owns character facts, submission requirements, fix safety, and
presentation.

```text
CLI ─────┐
         ├─> core ─> data
WASM ────┘
```

All workspace crates are private and `publish = false`.

| crate | responsibility |
|---|---|
| `aozora-proof-data` | build-time JIS, platform-dependent, orthography, and gaiji tables |
| `aozora-proof-core` | pure Rule Catalog, checks, fixes, and schema-v2 serialization |
| `aozora-proof-cli` | configuration, discovery, I/O, rendering, watch mode, and review TUI |
| `aozora-proof-wasm` | browser façade over the same catalog and machine report |

## Rule and requirement model

The Rule Catalog is the only source for a proof-owned code, category, default
severity, English/Japanese text, detection class, fix applicability, official
authority, and examples. Codes are
`aozora::proof::<category>::<rule>`; upstream parser codes remain
`aozora::lex::*`.

Official requirements are catalogued as `Automatic`, `Review`, or `Manual`.
The coverage test rejects an official item with no rule and a rule not
referenced by any official item. Manual requirements appear in machine
summaries and the review checklist, so `Report::conformant` means only that the
automatically decidable subset conforms.

## Pipeline

`run_submission_with_orthography(raw, policy)` performs:

1. raw-byte encoding, BOM, line-ending, and final-newline checks;
2. UTF-8 or Shift_JIS decoding into one decoded coordinate frame;
3. upstream notation diagnostics;
4. character, gaiji, contextual-review, and directional-orthography checks;
5. opening and closing submission-wrapper checks;
6. stable ordering by decoded byte span and rule code.

Each `Finding` has a decoded UTF-8 byte span. Unicode line and code-point
columns are derived from the decoded text during serialization. Raw-byte
operations such as BOM removal and encoding conversion remain structured fix
operations rather than pretending to be decoded text edits.

## Fix safety

`FixApplicability` is `Safe` or `Review`; absence of a fix represents `None`.
A safe operation has one meaning-independent result. The planner rejects
overlapping text edits, applies all operations in memory to a fixed point,
reruns the submission checks, and requires exact Shift_JIS round-trip
encoding. The CLI then preserves permissions and uses an atomic replacement
only when the on-disk bytes still match those originally read.

The TUI uses the same edit primitives but stores review decisions only in its
session. It prepares every changed file before writing any and reuses the
concurrent-change guard.

## Configuration and discovery

Configuration merges the platform user file and nearest project file before
environment and command flags are applied. Per-path overrides are evaluated
against normalized display paths. Unknown keys are rejected by TOML
deserialization and unknown rule codes are checked against the catalog.

Directory discovery returns normalized, deduplicated, sorted paths. Ignore
files affect discovered entries only; explicit file arguments bypass them.
Symlinked directories and hidden paths are not traversed.

## Machine contracts

Schema v2 is a top-level object:

```json
{
  "schemaVersion": 2,
  "tool": {"name": "aozora-proof", "version": "0.2.0"},
  "summary": {},
  "files": []
}
```

Files expose path, encoding, line ending, orthography, `conformant`,
`reviewPending`, and findings. Findings expose code, category, severity,
source, `utf8ByteSpan`, one-based Unicode position, canonical English message,
structured data, authority URL, and fix alternatives.

Compact JSON serialization plus stable input/finding ordering makes machine
output byte-stable. SARIF uses `unicodeCodePoints`, declares artifact encoding,
links rule help to its authority, and emits replacement regions for text
edits. Human output alone is localized and terminal-dependent.

## Distribution

Supported installations are GitHub Release archives and the composite GitHub
Action. Release automation builds five platform targets, generates the man
page and completions from Clap, writes per-archive and aggregate SHA-256
digests, emits an SPDX JSON SBOM, and creates GitHub provenance attestations.
The action downloads the selected archive and verifies both its digest and
attestation; it never invokes Cargo.

See [ADR 0005](docs/adr/0005-submission-proofreading-cli.md) for the governing
decision.

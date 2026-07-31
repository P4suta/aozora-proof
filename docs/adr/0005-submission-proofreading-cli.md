# 5. Submission-quality proofreading CLI

- Status: accepted
- Date: 2026-07-31
- Deciders: aozora-proof maintainers
- Tags: architecture, cli, wire, distribution

## Context

The original command exposes character checks and parser diagnostics, but it
does not model the full submission workflow. It has no explicit orthography
policy, no distinction between safe and judgement-dependent changes, and no
machine-readable account of official checks that remain manual.

The current findings envelope also describes an isolated list rather than a
submission. Consumers cannot reliably recover the input encoding, line
endings, review status, authority, Unicode position, or available edits.

## Decision

Version 0.2 treats `aozora-proof` as a submission-quality proofreading
application. `check` evaluates the complete catalog without writing, `fix`
applies only changes classified as `Safe`, and `review` stages
judgement-dependent changes in memory before an atomic write.

The rule catalog is the sole authority for stable code, category, severity,
localized presentation, detection class, fix applicability, official
authority, and examples. Proofreader-owned codes use
`aozora::proof::<category>::<rule>`. Parser diagnostics retain their upstream
`aozora::lex::*` identity.

Every official checklist or proofreading-manual item is classified as
automatic, review, or manual. Manual items remain visible in the catalog and
review checklist and never contribute to an automated-conformance claim.

Document commands require an orthography policy: `modern`, `traditional`, or
`mixed`. Resolution follows command flag, environment, nearest project
configuration, and user configuration. Non-interactive invocations fail with
a usage error when no policy is available.

Fixes are typed as `Safe`, `Review`, or `None`. A safe fix has one
meaning-independent replacement. A file's safe edits are rejected when they
overlap, applied in memory to a fixed point, reparsed, and encoded to
Shift_JIS without loss before one atomic replacement. Repeating `fix` must be
a no-op.

Machine reports use schema version 2. The top-level object contains
`schemaVersion`, tool metadata, summary, and files. File records contain path,
detected encoding and line ending, conformance and review state, and sorted
findings. Findings contain their code, category, severity, source, decoded
UTF-8 byte span, one-based Unicode position, canonical English message,
structured data, authority URL, and fix alternatives. JSON, short, and SARIF
are canonical English and deterministic; localization affects only human
output.

Internal crates remain `publish = false`. Supported distribution is a
standalone binary in GitHub Release archives and a GitHub Action that installs
a checksum-verified release asset. Archives also contain generated manual
pages and shell completions. No crates.io publishing path is introduced.

## Consequences

- Callers must migrate from schema version 1 and the pre-0.2 command surface.
- A single catalog keeps the CLI, WASM, web presentation, documentation, and
  official-coverage tests aligned.
- Safe automation stays deliberately narrow; more findings require review but
  cannot silently alter an author's text.
- Release automation must build, attest, checksum, and package platform
  binaries instead of publishing workspace crates.
- Full-screen review adds terminal-state and concurrent-write failure modes
  that require state-machine, snapshot, and pseudo-terminal tests.

## Alternatives considered

**Keep the character-checker interface.** Rejected because it cannot state
which submission requirements were evaluated and encourages consumers to
mistake a clean character scan for a complete proof.

**Autofix every suggestion.** Rejected because orthography, gaiji, OCR,
spacing, ruby, and bibliography changes depend on the base text or editorial
intent.

**Preserve schema version 1 additively.** Rejected because its list envelope
lacks a file-level submission model; compatibility would make optional the
fields required to interpret a result.

**Publish the internal crates.** Rejected in accordance with ADR 0004: the
experimental boundaries do not yet justify public Rust APIs or independent
release lifecycles.

## References

- [CLI Guidelines](https://clig.dev/)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/0.8/)
- [`git add -p`](https://git-scm.com/docs/git-add)
- [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html)
- [青空文庫 テキストファイルの作り方](https://www.aozora.gr.jp/KOSAKU/textfile_checklist/)
- [青空文庫 校正マニュアル](https://www.aozora.gr.jp/aozora-manual/index-proofreading.html)

# 6. Static Rust failure and safety invariants

- Status: accepted
- Date: 2026-07-31
- Deciders: aozora-proof maintainers
- Tags: architecture, reliability, tooling

## Context

The checker accepts untrusted documents and publishes deterministic results to
native, WASM, and web consumers. Several implementation shortcuts could turn
an incomplete check into an apparently clean report: failed parsing returned
no findings, missing rule metadata was treated as upstream data, invalid spans
were clamped, and JSON failures produced empty fallback envelopes.

Other invariants existed only through convention. Trait objects erased concrete
failure types, review candidates and decisions lived in parallel collections,
and unchecked arithmetic mixed statistical counters with source coordinates.
The existing strict lint policy did not cover these failure modes.

## Decision

Repository-owned Rust uses static dispatch and contains no explicit trait
objects. Generic parameters, concrete implementations, and enums provide
polymorphism. The `rust-policy` xtask parses Rust with `syn` and rejects
`TypeTraitObject` nodes. The local and hosted lint gates run that policy.

Checking returns `Result<Report, CheckError>`. Serialization returns the
serializer's concrete error. Decode, I/O, and configuration failures are usage
errors at the CLI boundary; engine invariants and serialization failures are
internal errors. WASM exports reject with a JavaScript exception, and the web
interface renders an error state distinct from an empty finding list.

Errors retain their sources in concrete enums. Parser failures, unknown rules,
invalid or unrepresentable spans, conflicting edits, non-convergent fixes, and
writer failures cannot be replaced with empty results, clamped coordinates, or
synthetic JSON.

Review state stores each candidate and decision in one item. Statistical
counters and capacity estimates saturate. Source spans, offsets, edits, and
state transitions use checked conversions and arithmetic, returning a concrete
error when they cannot be represented.

The workspace denies the targeted restriction lints that enforce these
decisions. The blanket `clippy::restriction` group remains disabled because it
also rejects project conventions without identifying a safety invariant.

## Consequences

- Callers must handle checker and serializer failures explicitly.
- A report means every enabled stage completed; an empty report no longer
  represents an unknown or failed check.
- New explicit trait objects fail before ordinary compilation in the policy
  gate.
- Adding a rule, coordinate transformation, or review transition requires a
  checked representation of its failure mode.
- Targeted restriction lints may require justified test-only allowances, but
  unrelated stylistic restrictions do not become workspace policy.

## Alternatives considered

**Keep fallbacks and emit internal findings.** Rejected because decoding,
parsing, and serialization can fail before a trustworthy finding or envelope
exists.

**Adopt the complete restriction lint group.** Rejected because it warns on
valid error propagation and project text while obscuring the checks tied to
actual failure modes.

**Search source text for `dyn`.** Rejected because comments, strings, and token
layout produce false results. The AST policy identifies the Rust type construct
regardless of nesting or formatting.

**Keep parallel review collections with length assertions.** Rejected because
the type can make the invalid state unrepresentable.

## References

- [ADR 0002](0002-strict-lint-policy.md)
- [ADR 0005](0005-submission-proofreading-cli.md)
- `CheckError`
- `FixError`
- `ReviewItem`
- `cargo xtask lint rust-policy`

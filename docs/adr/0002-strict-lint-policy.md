# 2. Strict deny-level lint policy

- Status: accepted
- Date: 2026-06-21
- Deciders: aozora-proof maintainers
- Tags: process, quality, tooling

## Context

aozora-proof is a proofreading tool: its whole value is telling an author
which characters in their 青空文庫 text are non-conformant. A false negative
(missing a real defect) or a panic on adversarial input erodes the one thing
the tool sells — trust in its verdict. The engine is also WASM-clean and runs
unattended in CI, where a panic is a hard failure, not a recoverable error.

Rust's default lints catch little of what bites a correctness-sensitive
library: silent `unwrap()` / `expect()` panics, `panic!` / `todo!` /
`unimplemented!` left on shipping paths, `a[i]` indexing that can panic on
attacker-controlled offsets, lossy `as` casts, undocumented public items. The
sibling ecosystem repos run these at `warn`, where they scroll past in CI
output and accumulate.

## Decision

Run the strictest practical lint set across the whole workspace, at `deny`,
enforced in CI with `-D warnings`. The configuration lives in one place,
`[workspace.lints]` in `Cargo.toml`:

- `unsafe_code = "forbid"` — the engine is pure and WASM-clean; no `unsafe`.
- clippy `pedantic`, `nursery`, and `cargo` groups at `deny`.
- Restriction lints that turn latent panics into compile errors:
  `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`,
  `unreachable`, `indexing_slicing`, `string_slice`, `as_conversions`.
- `missing_docs` plus the rustdoc link lints — every public item is documented.

The escape hatch stays available but must be justified:
`allow_attributes_without_reason = "deny"` forces every `#[allow(...)]` to
carry a `reason = "…"`, so each suppression is a deliberate, reviewable
decision rather than silent debt.

Two lints are allowed workspace-wide, each with a recorded rationale:
`module_name_repetitions` (renaming well-named types like `Finding` would be
worse) and `multiple_crate_versions` (transitive, outside our control).

## Consequences

- Whole classes of defect — panics, undocumented APIs, lossy casts — fail the
  build instead of reaching a release. For a correctness tool, that is the
  point.
- Writing code costs more: panicking shortcuts (`unwrap`, `a[i]`) must be
  rewritten as explicit `Result` / `get()` handling. We accept the friction as
  the price of the guarantee.
- Every suppression carries a `reason = "…"`, so it surfaces in review and in
  `git grep`; the policy degrades visibly rather than silently.
- New `nursery` / `pedantic` lints can break the build on a toolchain bump.
  The toolchain is pinned (`rust-toolchain.toml`), so bumps are deliberate and
  the breakage is absorbed in the bump PR.

## Alternatives considered

- **Warn-level lints (the sibling-repo default).** Rejected: warnings scroll
  past in CI and pile up; nothing forces them to zero, so the signal erodes.
  `deny` is the only level that holds the line.
- **Rust defaults only.** Rejected: the defaults miss exactly the
  panic / indexing / cast hazards that matter most for a tool parsing
  untrusted text.
- **Per-crate lint config.** Rejected: `[workspace.lints]` keeps one policy in
  one place; per-crate drift is how a standard rots.

## References

- `Cargo.toml` `[workspace.lints]` — the enforced configuration.
- `CONTRIBUTING.md` — the "No warning suppressions" contributor rule.
- Issue #19 — record this stance as an ADR so the sibling repos can reference
  it when deciding whether to adopt the same.

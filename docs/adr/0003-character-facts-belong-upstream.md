# 3. Character-fact data belongs to `aozora`, not `aozora-proof`

- Status: accepted
- Date: 2026-06-21
- Deciders: aozora-proof maintainers
- Tags: architecture, packaging

## Context

`aozora-proof` is a proofreader built on the sibling
[`aozora`](https://github.com/P4suta/aozora) parser. `ARCHITECTURE.md` already
splits responsibility once: the **notation level** (ruby, bouten, 外字
resolution, bracket pairing, structured diagnostics) belongs upstream — *"we
consume it; we do not reimplement it"* — and the **character level** belongs
here.

But the character level bundles two different kinds of thing:

- **Facts** — objective data about the writing system and JIS: 水準
  classification, 機種依存文字, 面区点, the 外字注記辞書, the 旧字体→新字体 table.
  Today these live in a separate crate, `aozora-proof-data`. Reading it
  (`crates/aozora-proof-data/src/lib.rs`), *every* item is a fact — `jis_level`,
  `is_platform_dependent`, `shinji_for`, `men_ku_ten` / `char_at_men_ku_ten`,
  `gaiji_descriptions` / `gaiji_search`. None of it encodes a severity, a
  message, or a suggestion.
- **Policy** — which facts are worth reporting and at what severity, the rule
  identifiers, the suggestions, and the CLI / WASM / web product that delivers
  them.

As we approach crates.io publishing (#9) at v0.x, the fact layer becomes a
problem. `aozora-proof-data` would publish a second crate that owns JIS / gaiji
reference data which `aozora` already conceptually owns — and which other
consumers want too: a renderer (`aozora-render`) needs the same 外字 mapping to
render 外字注記, and an editor server (#12) needs the same classification. Two
crates owning the same tables means version skew and no single source of truth.
Pre-1.0 is the moment to settle the boundary, before a publish fixes it.

## Decision

Draw the boundary at **facts vs policy**. The test: *would a renderer or an LSP
also want this knowledge?* If yes, it is a fact and belongs upstream in
`aozora`. If it encodes a severity, message, suggestion, or product UX, it is
policy and stays here.

Concretely:

- **Migrate the entire `aozora-proof-data` crate upstream** into `aozora`
  (proposed home: the `aozora-spec` sub-crate, surfaced as `aozora::spec::*` /
  `aozora::gaiji::*`). That includes `jis_level` / `Suijun`,
  `is_platform_dependent`, `shinji_for`, `men_ku_ten` / `char_at_men_ku_ten`,
  the 外字注記辞書 lookups, and the vendored sources with their `NOTICE`
  attributions. `aozora-proof-data` then **ceases to exist**, and
  `aozora-proof-core` depends on `aozora` alone.
- **`aozora-proof` keeps the policy and the product**: the `moji` / `kyuji`
  severity cascades, `gaiji_dict::annotate` suggestion logic, the `rules` /
  `finding` / `pipeline` / `coords` engine, the CLI / WASM / web surface, and
  the output formats. These call the upstream facts; they do not own them.
- **The finding-code namespace** (`aozora::char::*`, `aozora::kyuji::*`,
  `aozora::gaiji::*` in `finding.rs`) is `aozora-proof`'s stable **lint-rule
  identity**, owned here — it is a string contract, distinct from any `aozora`
  Rust module path that happens to read the same. Keep the strings; this ADR
  records that `aozora-proof` owns them. (They name "rules about the aozora
  character layer," not the upstream module.)

Execution is sequenced and cross-repo: upstream adds the API and publishes first
(#26), then this repo deletes the data crate and repoints `core` (#27), with the
existing conformance tests pinning behavior-equivalence.

## Consequences

- One source of truth for writing-system facts. A renderer, an LSP (#12), and a
  converter consume the same tables; if `aozora-render` already vendors JIS /
  gaiji data, this deduplicates the ecosystem rather than merely relocating.
- No overlapping `aozora-proof-data` crate on crates.io; `aozora-proof-core`
  gains a single clean dependency. This unblocks the publishing story (#9).
- `aozora-proof` shrinks to its true essence — an opinionated linter plus a
  product — mirroring the `clippy` / `rustc` split: the parser is not burdened
  with lint policy or a product surface.
- The cost is a cross-repo migration with a publish ordering: `aozora` must ship
  the API first, and behavior-equivalence must be *proven* (conformance and
  foundation tests unchanged), not assumed.

## Alternatives considered

- **Status quo — publish `aozora-proof-data` as-is.** Rejected: it ships a
  second crate owning JIS / gaiji reference data that `aozora` already
  conceptually owns, inviting version skew and an ambiguous source of truth —
  and the choice is hard to walk back after 1.0.
- **Full absorption — fold all proofreading into `aozora` and archive this
  repo.** Rejected: it couples a parser (a mechanism) to a lint policy and a
  CLI / web / CI product (a fast-moving, opinionated thing). That is a category
  error — `clippy` is deliberately *not* part of `rustc` for the same reason —
  and it discards `aozora-proof`'s actual value (the opinionated tool) to
  relocate the one part (data) that should move anyway.
- **Monorepo — one repo, separate crates.** Rejected *for this decision*: it
  addresses maintenance overhead, a different axis than the facts/policy
  boundary. What matters here is dependency direction and crate ownership, which
  are settled independently of repo layout. Revisit separately if
  solo-maintenance cost becomes the driver.

## References

- `ARCHITECTURE.md` — the notation/character split this ADR extends to the
  character-fact layer.
- `crates/aozora-proof-data/src/lib.rs` — the facts being migrated.
- #26 — upstream the character-fact tables into `aozora` (precondition).
- #27 — retire `aozora-proof-data`; consume the facts from `aozora`.
- #9 — publishing & release automation, for which this decision is a
  precondition; #12 — `aozora-lsp`, a future consumer of the upstream facts.

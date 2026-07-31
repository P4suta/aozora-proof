# 4. Promote character facts only after demonstrated demand

- Status: accepted
- Date: 2026-07-31
- Deciders: aozora-proof maintainers
- Tags: architecture, packaging, experimentation
- Supersedes: [ADR 0003](0003-character-facts-belong-upstream.md)

## Context

ADR 0003 correctly separated objective character facts from proofreading
policy, but coupled that boundary to an immediate migration of every fact into
the public `aozora` API. The migration would create a SemVer commitment before
the corpus audit has shown which facts are useful outside this proofreader.

`aozora-proof` is not published to a package registry. Its current data crate is
therefore a cheap experimental boundary: tables and queries can change while
the rules are validated against released works and synthetic defects.

## Decision

Keep the facts-versus-policy distinction from ADR 0003, but do not migrate the
whole fact layer in advance.

Character facts remain in the unpublished `aozora-proof-data` crate until all
of these conditions hold:

1. the value is independent of severity, wording, suggestions, and product UX;
2. an implemented second consumer outside `aozora-proof` needs the same
   semantics;
3. provenance and behavior are protected by conformance tests;
4. the required public surface can be reviewed and released independently.

Only the facts meeting those conditions are promoted. The remaining data stays
experimental. The parser does not gain proofreading policy, and the
proofreader does not duplicate notation parsing.

The core, data, and CLI packages stay non-publishable during the validation
campaign. Corpus auditing is a development command rather than a supported CLI
or wire contract.

## Consequences

- `aozora` gains no speculative public API.
- Corpus evidence can change the fact model without a compatibility burden.
- A future renderer or editor can still trigger a narrow upstream promotion.
- Registry publishing needs a separate explicit decision after the validation
  campaign.
- The eager migrations tracked by #26 and #27 are no longer current work.

## Alternatives considered

- **Execute ADR 0003 immediately.** Rejected because it fixes API shape before
  consumer demand and corpus behavior are known.
- **Keep every fact in this repository permanently.** Rejected because a real
  second consumer may later justify one canonical implementation.
- **Move proofreading policy into `aozora`.** Rejected because policy changes
  faster than parsing and is not a notation fact.

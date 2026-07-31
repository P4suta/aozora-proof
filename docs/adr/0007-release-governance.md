# 7. Govern releases as one qualified product

- Status: accepted
- Date: 2026-07-31
- Deciders: aozora-proof maintainers
- Tags: release, supply-chain, distribution

## Context

The repository had not published a release, although development files called
the current work 0.2.0. Its tag-triggered workflow rebuilt binaries after the
qualification point and could publish any manually created `v*` tag. Version
references were duplicated across Cargo metadata, documentation, the composite
Action, and lockfiles.

The Rust crates are implementation boundaries rather than independently
supported packages. The supported product consists of the native CLI, the
GitHub Action that installs it, and the Web/Pages deployment built from the
same published commit.

## Decision

The unchanged initial product is released as v0.1.0. The 0.2 version language
in ADR 0005 described the then-unpublished redesign; this decision overrides
only those version-number statements. ADR 0005 remains otherwise accepted and
unchanged. Machine report `schemaVersion: 2` is an independent compatibility
contract and is not renumbered.

`version.txt` is the release ledger. Release Please treats the repository root
as one `simple` component and maintains its Release PR, manifest, and
`CHANGELOG.md`. Before 1.0, breaking changes increment the minor version while
ordinary features and fixes increment the patch version. Release Please never
creates tags or GitHub Releases.

The Release PR synchronizes the Cargo workspace version, internal dependency
requirements, both Cargo lockfiles, and the documented Action reference. All
Rust crates remain `publish = false`; no crates.io publication credential or
workflow is introduced.

`release-ready` qualifies a Release Please PR and its version-changing merge
commit on five native runners. A successful main-branch qualification is the
only event that dispatches publication. The release workflow reuses the exact
qualification artifact, creates a tag and complete draft with a GitHub App
token, and never rebuilds the CLI. A tag-scoped, Environment-approved job
rechecks the tag, manifest, checksums, and attestations before publishing.
Pages and rustdoc are dispatched only after the immutable release is public.

## Consequences

- Normal pushes explicitly pass `release-ready` as a no-op and cannot publish.
- Manually created tags do not trigger publication.
- A recovery run names a full commit and defaults to read-only dry-run mode.
- Draft creation, asset attachment, and publication remain distinct so GitHub
  release immutability can be enabled safely.
- Repository rulesets, Environment policies, and App permissions are part of
  the release contract and are checked by `just release-preflight`.

## Alternatives considered

**Publish each crate.** Rejected because it creates unsupported API and
dependency lifecycles without a consumer requirement.

**Let Release Please create the GitHub Release.** Rejected because the tag
would precede native qualification and asset verification.

**Build again after tagging.** Rejected because it would publish artifacts
other than the ones tested on the candidate commit.

## References

- [Release Please Action](https://github.com/googleapis/release-please-action)
- [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)

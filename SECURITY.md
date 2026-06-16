# Security Policy

## Reporting a vulnerability

Please do **not** open a public issue for security reports. Instead open a
private advisory at
<https://github.com/P4suta/aozora-proof/security/advisories/new>.

Include where possible:

- the shortest input / steps that reproduce the issue,
- the version or commit hash and your Rust toolchain version,
- whether the issue is reachable from untrusted input.

## Scope

`aozora-proof` parses and classifies untrusted text. Of particular interest:
panics, unbounded memory or time, or a misclassification that could let a CI
gate pass malformed text. The notation parser itself lives in the sibling
[`aozora`](https://github.com/P4suta/aozora) repository — parser-level issues
belong there.

## Response

- Acknowledge within ~7 days.
- Coordinated fix and disclosure, typically within 30–60 days (faster for
  critical issues).
- Credit in the changelog once a fix ships, unless you prefer otherwise.

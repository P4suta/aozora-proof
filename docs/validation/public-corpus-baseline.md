# Public corpus baseline

The first validation run used the public
[`aozorabunko_text`](https://github.com/aozorabunko/aozorabunko_text)
mirror at commit `b1ec9a7fa46de8dd5acc33378428c899e86bfb32`.

| Measure | Result |
|---|---:|
| `.txt` files | 17,889 |
| Source bytes | 845,727,350 |
| Internal diagnostics | 0 |
| Control characters | 89 findings in 12 files |
| Half-width katakana | 25 findings in 19 files |
| Platform-dependent characters | 10 findings in 6 files |
| Characters outside JIS X 0213 | 1 finding in 1 file |
| Mixed line endings | 123 files |
| Other non-CRLF line endings | 4 files |

The mirror stores UTF-8, so every non-ASCII file receives the submission-only
UTF-8 note. That result validates the check but does not describe the encoding
of files accepted by the official workflow.

The first run reported 638,652 platform-dependent characters in 10,295 files.
Inspection showed that the generated classifier retained the primary Unicode
mapping for a JIS cell but discarded its Windows mapping alias. Registering
both aliases reduced that result to 10 findings without weakening known
platform-dependent cases such as circled numbers.

The audit-only candidate rules produced:

| Candidate | Findings | Files |
|---|---:|---:|
| Half-width space | 6,728 | 875 |
| ASCII parenthesis | 1,685 | 215 |
| Full-width tilde | 3,126 | 1,276 |

These candidates remain experimental. Their frequency and dependence on
notation context make them unsuitable as default warnings without manual
classification.

This baseline is evidence for continued validation, not a release decision or
a claim that every finding is a manuscript defect. The aggregate report is
regenerated locally with `just audit-corpus`; corpus text and path-bearing
reports are not committed.

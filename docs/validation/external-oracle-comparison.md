# External oracle comparison

[`AozoraBunko::Checkerkun`](https://github.com/pawa-/AozoraBunko-Checkerkun)
is used as an independent behavioral oracle. Its implementation and character
tables are not copied into this repository.

The documented examples establish shared expectations for JIS外字, half-width
katakana, platform-dependent characters, control characters, and optional
old/new-form checks. Equivalent synthetic cases are protected by
`external_oracle.rs` and the mutation corpus.

The tools deliberately differ at the product boundary:

| Behavior | Checkerkun | aozora-proof |
|---|---|---|
| JIS外字 and half-width katakana | Default checks | Default checks |
| Control characters | Marked in output | Error finding |
| Half-width spaces, ASCII parentheses, full-width tilde | Default checks | Audit-only candidates |
| Old/new-form characters | Optional checker | Note with suggestion |
| Source modification | Annotated transformed output | Read-only report and diff preview |
| Aozora notation structure | Character-oriented exceptions | Parsed spans from `aozora` |

The public corpus shows why the differing defaults matter: the three
audit-only character classes occur in hundreds of released files. They need
contextual review before becoming user-facing warnings.

The comparison is semantic rather than byte-for-byte. A disagreement is
classified under the corpus audit protocol before changing a default rule.

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
| Half-width spaces and ASCII parentheses | Default checks | Contextual review findings |
| Full-width tilde | Default check | Audit-only candidate |
| Old/new-form characters | Optional checker | Directional review alternatives |
| Source modification | Annotated transformed output | Typed safe fix or confirmed review edit |
| Aozora notation structure | Character-oriented exceptions | Parsed spans from `aozora` |

The public corpus shows why the differing defaults matter: broad character
classes occur in hundreds of released files. Version 0.1 narrows its
user-facing spacing rule by neighboring script and keeps every result in the
review class.

The comparison is semantic rather than byte-for-byte. A disagreement is
classified under the corpus audit protocol before changing a default rule.

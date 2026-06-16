# Contributing to aozora-proof

Thanks for your interest! This is a small, focused FOSS project; contributions
of all sizes are welcome.

## Ground rules

- **Host toolchain.** Everything runs on host `cargo` — no Docker required. The
  toolchain is pinned by [`rust-toolchain.toml`](./rust-toolchain.toml) (1.95.0).
- **No warning suppressions.** An `#[allow(...)]` needs a `reason = "…"`; CI runs
  clippy with `-D warnings`.
- **The notation level belongs upstream.** `aozora-proof` consumes the
  [`aozora`](https://github.com/P4suta/aozora) parser (pinned by release tag)
  for ruby / bouten / 外字 resolution / diagnostics. Parser changes land there,
  not here; this repo owns the **character level** only.
- **Conventional Commits**, enforced by the `commit-msg` hook.

## Setup

```console
$ rustup show                 # picks up rust-toolchain.toml
$ lefthook install            # pre-commit / pre-push hooks (recommended)
$ cargo test --workspace
```

## Development loop

```console
$ cargo check  --workspace --all-targets        # fast
$ cargo test   --workspace --all-targets
$ cargo clippy --workspace --all-targets -- -D warnings
$ cargo fmt --all
$ just ci                                        # everything CI runs
```

`bacon` (`bacon` / `bacon clippy` / `bacon test`) gives a fast watch loop.

## Pull requests

- Add or update tests for behaviour changes.
- Keep `cargo clippy … -D warnings` and `cargo fmt --all -- --check` clean.
- Add a `CHANGELOG.md` entry under `[Unreleased]`.
- PRs are reviewed via [`CODEOWNERS`](./.github/CODEOWNERS).

## License

By contributing you agree your work is licensed under **Apache-2.0 OR MIT**.

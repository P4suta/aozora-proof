# Contributing to aozora-proof

Thanks for your interest! This is a small, focused FOSS project; contributions
of all sizes are welcome.

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the pieces fit together.

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

One command bootstraps everything — toolchain components, dev tools, git hooks:

```console
$ ./bootstrap.sh      # installs cargo-binstall + just, then runs `just setup`
$ just doctor         # verify your toolchain + tools match what CI pins
```

Already have `just`? `just setup` alone does the same. The pieces are also
available individually: `just setup-toolchain`, `just setup-tools`, `just hooks`.
Cargo tool versions are pinned in [`dev-tools.txt`](./dev-tools.txt). The non-cargo
lint tools (`actionlint`, `shellcheck`) come from mise or your package manager;
CI installs them via `taiki-e/install-action`. The web app (`web/`) is a SvelteKit
project — install Node 24+ and `pnpm`, then `pnpm -C web install`.

## Development loop

```console
$ just check          # fast "still compiles?"
$ just test           # tests + doctests (nextest when present, like CI)
$ just clippy         # -D warnings, like CI
$ just fmt            # auto-format
$ just ci             # everything CI's gating jobs run
$ just ci-full        # + the coverage job (full CI parity)
```

`just --list` shows every recipe. `bacon` (`bacon` / `bacon clippy` /
`bacon nextest`) gives a fast watch loop. `just doctor` reports whether your
local tools match the versions CI pins.

Hacking on the **web app**? After `pnpm -C web install`, `just serve` builds the
WASM package and runs the SvelteKit dev server at <http://localhost:5173>; `just
web` adds a watcher that rebuilds the WASM on Rust changes. `just lint-web` runs
the full web gate (prettier + eslint + svelte-check + prerender build).

## Troubleshooting

- **clippy/fmt passes locally but fails in CI** — your `rustc` may differ from
  the pinned channel. Run `just doctor`; `rustup show` re-syncs to
  `rust-toolchain.toml` (unset `RUSTUP_TOOLCHAIN` if it is set).
- **`typos` flags a domain term** — add it to `_typos.toml` under
  `[default.extend-words]`.
- **`mold: linker not found`** — mold is optional (see `.cargo/config.toml`);
  unset the linker `RUSTFLAGS` or install mold.
- **Can't reproduce a CI failure locally** — `just ci-full` runs every CI job
  (nextest + lint + deny + coverage) in one shot.
- **web app blank / "module" error** — run it through the dev server, not
  `file://`: `pnpm -C web install` then `just serve` (Vite dev) or `just web`.

## Pull requests

- Add or update tests for behaviour changes.
- Keep `cargo clippy … -D warnings` and `cargo fmt --all -- --check` clean.
- Add a `CHANGELOG.md` entry under `[Unreleased]`.
- PRs are reviewed via [`CODEOWNERS`](./.github/CODEOWNERS).

## License

By contributing you agree your work is licensed under **Apache-2.0 OR MIT**.

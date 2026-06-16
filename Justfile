# aozora-proof — host-cargo dev commands. `just --list` shows them all.
set shell := ["bash", "-c"]

default:
    @just --list

# ---- bootstrap -------------------------------------------------------------

# one-command contributor bootstrap: toolchain, dev tools, git hooks.
setup: setup-toolchain setup-tools hooks
    @echo "setup complete — try 'just doctor', then 'just ci'."

# materialise the pinned toolchain + the wasm target the web app needs.
setup-toolchain:
    rustup show
    rustup target add wasm32-unknown-unknown

# install the pinned dev tools fast via cargo-binstall (prebuilt binaries).
setup-tools:
    command -v cargo-binstall >/dev/null 2>&1 || cargo install cargo-binstall --locked
    sed -E 's/#.*//' dev-tools.txt | xargs cargo binstall --no-confirm
    command -v lefthook >/dev/null 2>&1 || cargo binstall --no-confirm lefthook

# report dev-tool + toolchain status against what CI pins (read-only).
doctor:
    #!/usr/bin/env bash
    pin=$(sed -n 's/.*channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml)
    active=$(rustc --version | cut -d' ' -f2)
    echo "toolchain pin (rust-toolchain.toml): ${pin:-unknown}"
    echo "active rustc                       : ${active:-unknown}"
    [ "$pin" = "$active" ] || echo "  ⚠  active rustc differs from the pin — run 'rustup show' (unset RUSTUP_TOOLCHAIN if it is set)"
    echo "dev tools:"
    for t in cargo-nextest cargo-deny cargo-llvm-cov wasm-pack typos bacon lefthook just; do
      if command -v "$t" >/dev/null 2>&1; then
        ver=$("$t" --version 2>/dev/null | head -1)
        printf '  ✓ %-14s %s\n' "$t" "${ver:-installed}"
      else
        printf '  ✗ %-14s missing — run: just setup\n' "$t"
      fi
    done
    if command -v mold >/dev/null 2>&1; then echo "  ✓ mold (optional, faster linking)"; else echo "  · mold not found (optional; see .cargo/config.toml)"; fi
    if [ -f .git/hooks/pre-commit ]; then echo "  ✓ git hooks installed"; else echo "  ✗ git hooks not installed — run: just hooks"; fi

# ---- inner loop ------------------------------------------------------------

# fast "still compiles?" gate
check *ARGS:
    cargo check --workspace --all-targets {{ ARGS }}

build *ARGS:
    cargo build --workspace --all-targets {{ ARGS }}

# tests + doctests. Uses cargo-nextest (like CI) when present, else plain cargo.
test *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo nextest run --workspace --all-targets {{ ARGS }}
    else
        echo "note: cargo-nextest not found — using plain 'cargo test' (run 'just setup' for CI parity)"
        cargo test --workspace --all-targets {{ ARGS }}
    fi
    cargo test --workspace --doc

# explicit plain-cargo test path (no nextest), for minimal environments.
test-portable *ARGS:
    cargo test --workspace --all-targets {{ ARGS }}
    cargo test --workspace --doc

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items

deny:
    cargo deny --all-features check

typos:
    typos

cov:
    cargo llvm-cov --workspace --summary-only

# everything CI's gating jobs run (test + lint + deny). Mirrors ci.yml.
ci: fmt-check clippy test doc deny typos
    @echo "ci: gating checks passed (use 'just ci-full' to also run the coverage job)"

# full CI parity — also reproduces the coverage job (ci.yml `coverage`).
ci-full: ci cov
    @echo "ci-full: all CI jobs reproduced locally"

# install the lefthook git hooks
hooks:
    lefthook install

#!/usr/bin/env bash
# One-time contributor seed. Installs the two tools needed to bootstrap —
# `cargo-binstall` (fast prebuilt installs) and `just` (the task runner) —
# then hands off to `just setup`, which installs the rest of dev-tools.txt
# and the git hooks. Idempotent: re-running is a no-op once provisioned.
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: Rust/cargo not found. Install the toolchain first: https://rustup.rs" >&2
    exit 1
fi

command -v cargo-binstall >/dev/null 2>&1 || cargo install cargo-binstall --locked
command -v just           >/dev/null 2>&1 || cargo binstall --no-confirm just

echo "seed complete — running 'just setup'…"
exec just setup

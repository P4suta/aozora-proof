#!/usr/bin/env bash
# One-time contributor seed. mise is the only prerequisite; the checked-in
# manifests and mise.lock install the exact Rust and development toolchain.
set -euo pipefail

if ! command -v mise >/dev/null 2>&1; then
    echo "error: mise not found. Install it from https://mise.jdx.dev/getting-started.html" >&2
    exit 1
fi

MISE_LOCKED=1 mise install just
echo "seed complete — running the locked setup…"
exec mise exec -- just setup

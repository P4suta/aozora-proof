# aozora-proof — host-cargo dev commands. `just --list` shows them all.
set shell := ["bash", "-c"]

default:
    @just --list

# fast "still compiles?" gate
check *ARGS:
    cargo check --workspace --all-targets {{ ARGS }}

build *ARGS:
    cargo build --workspace --all-targets {{ ARGS }}

# tests + doctests (portable: plain cargo, no nextest required)
test *ARGS:
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

# everything CI runs, in one shot
ci: fmt-check clippy test doc deny
    @echo "ci: all gates passed"

# install the lefthook git hooks
hooks:
    lefthook install

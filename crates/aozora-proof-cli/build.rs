//! Bake a richer `--version` string: crate version + git SHA + commit date +
//! target triple. Falls back gracefully (e.g. when built from a crates.io
//! tarball with no `.git`) to just the crate version + target.

use std::process::Command;

fn main() {
    let pkg = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let target = std::env::var("TARGET").unwrap_or_default();
    let sha = git(&["rev-parse", "--short", "HEAD"]);
    let date = git(&["show", "-s", "--format=%cs", "HEAD"]);

    let long = match (sha, date) {
        (Some(sha), Some(date)) => format!("{pkg} ({sha} {date} {target})"),
        (Some(sha), None) => format!("{pkg} ({sha} {target})"),
        _ => format!("{pkg} ({target})"),
    };

    println!("cargo:rustc-env=AOZORA_PROOF_LONG_VERSION={long}");
    // Re-stamp when HEAD moves (no-op when packaged without a .git directory).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}

/// Run `git <args>` in the crate dir; `None` if git is absent or fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

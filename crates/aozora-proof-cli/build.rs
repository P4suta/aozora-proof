//! Bake the crate version, source revision, commit date, and target triple into
//! the native command's long version string.

use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus};

#[derive(Debug, thiserror::Error)]
enum BuildError {
    #[error("required Cargo environment variable {name} is unavailable")]
    Environment {
        name: &'static str,
        #[source]
        source: env::VarError,
    },
    #[error("could not execute git while reading build metadata")]
    GitIo {
        #[source]
        source: std::io::Error,
    },
    #[error("git failed while reading {field} with status {status}")]
    GitStatus {
        field: &'static str,
        status: ExitStatus,
    },
    #[error("git returned non-UTF-8 {field}")]
    GitUtf8 {
        field: &'static str,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("git returned empty {field}")]
    EmptyGitMetadata { field: &'static str },
}

fn main() -> Result<(), BuildError> {
    let package = required_env("CARGO_PKG_VERSION")?;
    let target = required_env("TARGET")?;
    let checkout = Path::new("../../.git").exists();
    let revision = checkout
        .then(|| git("revision", &["rev-parse", "--short", "HEAD"]))
        .transpose()?;
    let date = checkout
        .then(|| git("commit date", &["show", "-s", "--format=%cs", "HEAD"]))
        .transpose()?;

    let long = match (revision, date) {
        (Some(revision), Some(date)) => format!("{package} ({revision} {date} {target})"),
        (None, None) => format!("{package} ({target})"),
        (Some(_), None) | (None, Some(_)) => {
            return Err(BuildError::EmptyGitMetadata {
                field: "revision tuple",
            });
        }
    };

    println!("cargo:rustc-env=AOZORA_PROOF_LONG_VERSION={long}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    Ok(())
}

fn required_env(name: &'static str) -> Result<String, BuildError> {
    env::var(name).map_err(|source| BuildError::Environment { name, source })
}

fn git(field: &'static str, args: &[&str]) -> Result<String, BuildError> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|source| BuildError::GitIo { source })?;
    if !output.status.success() {
        return Err(BuildError::GitStatus {
            field,
            status: output.status,
        });
    }
    let text =
        String::from_utf8(output.stdout).map_err(|source| BuildError::GitUtf8 { field, source })?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(BuildError::EmptyGitMetadata { field });
    }
    Ok(trimmed.to_owned())
}

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use aozora_proof_core::{FixError, apply_safe};
use similar::TextDiff;
use tempfile::NamedTempFile;

use crate::config::Resolved;
use crate::document::Document;

#[derive(Debug)]
pub(crate) struct FixOutput {
    pub(crate) stdout: Vec<u8>,
    pub(crate) changed_files: usize,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FixCommandError {
    #[error("{label}: autofix is disabled by configuration")]
    AutofixDisabled { label: String },
    #[error("{label}: orthography is required")]
    MissingOrthography { label: String },
    #[error("{label}: safe fixes could not be applied: {source}")]
    Fix {
        label: String,
        #[source]
        source: FixError,
    },
    #[error("{}: could not {operation}: {source}", path.display())]
    Io {
        path: PathBuf,
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("{}: target file has no parent directory", path.display())]
    NoParent { path: PathBuf },
    #[error("{}: file changed after it was read; refusing to overwrite", path.display())]
    ConcurrentChange { path: PathBuf },
    #[error("{}: could not atomically replace the target: {source}", path.display())]
    Persist {
        path: PathBuf,
        #[source]
        source: tempfile::PersistError,
    },
}

impl FixCommandError {
    pub(crate) const fn is_internal(&self) -> bool {
        matches!(self, Self::Fix { source, .. } if source.is_internal())
    }
}

pub(crate) fn run(
    documents: &[Document],
    settings: &Resolved,
    dry_run: bool,
) -> Result<FixOutput, FixCommandError> {
    let mut stdout = Vec::new();
    let mut changed_files = 0usize;
    for document in documents {
        if !settings.autofix_for(&document.label) {
            return Err(FixCommandError::AutofixDisabled {
                label: document.label.clone(),
            });
        }
        let Some(orthography) = settings.orthography_for(&document.label) else {
            return Err(FixCommandError::MissingOrthography {
                label: document.label.clone(),
            });
        };
        let fixed =
            apply_safe(&document.raw, orthography).map_err(|source| FixCommandError::Fix {
                label: document.label.clone(),
                source,
            })?;
        if !fixed.changed {
            if document.path.is_none() && !dry_run {
                stdout.extend_from_slice(&fixed.bytes);
            }
            continue;
        }
        changed_files = changed_files.saturating_add(1);
        if dry_run {
            stdout.extend_from_slice(
                unified_diff(&document.label, &document.report.decoded, &fixed.decoded).as_bytes(),
            );
            continue;
        }
        if let Some(path) = &document.path {
            atomic_write(path, &document.raw, &fixed.bytes)?;
        } else {
            stdout.extend_from_slice(&fixed.bytes);
        }
    }
    Ok(FixOutput {
        stdout,
        changed_files,
    })
}

pub(crate) fn unified_diff(label: &str, before: &str, after: &str) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&format!("a/{label}"), &format!("b/{label}"))
        .to_string()
}

pub(crate) fn atomic_write(
    path: &Path,
    original: &[u8],
    replacement: &[u8],
) -> Result<(), FixCommandError> {
    let metadata = fs::metadata(path).map_err(|source| io_error(path, "read metadata", source))?;
    let parent = path.parent().ok_or_else(|| FixCommandError::NoParent {
        path: path.to_path_buf(),
    })?;
    let mut temporary = NamedTempFile::new_in(parent)
        .map_err(|source| io_error(path, "create a temporary file", source))?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|source| io_error(path, "copy file permissions", source))?;
    temporary
        .write_all(replacement)
        .map_err(|source| io_error(path, "write replacement bytes", source))?;
    temporary
        .flush()
        .map_err(|source| io_error(path, "flush replacement bytes", source))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| io_error(path, "synchronize replacement bytes", source))?;
    let current = fs::read(path).map_err(|source| io_error(path, "reread input", source))?;
    if current != original {
        return Err(FixCommandError::ConcurrentChange {
            path: path.to_path_buf(),
        });
    }
    temporary
        .persist(path)
        .map_err(|source| FixCommandError::Persist {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn io_error(path: &Path, operation: &'static str, source: std::io::Error) -> FixCommandError {
    FixCommandError::Io {
        path: path.to_path_buf(),
        operation,
        source,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn concurrent_change_is_rejected_without_overwrite() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("work.txt");
        fs::write(&path, b"original").expect("write original");
        fs::write(&path, b"external").expect("write concurrent contents");

        assert!(matches!(
            atomic_write(&path, b"original", b"replacement"),
            Err(FixCommandError::ConcurrentChange { .. })
        ));
        assert_eq!(fs::read(path).expect("read file"), b"external");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("work.txt");
        fs::write(&path, b"original").expect("write original");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set permissions");

        atomic_write(&path, b"original", b"replacement").expect("atomic write");

        assert_eq!(
            fs::metadata(path)
                .expect("read metadata")
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }
}

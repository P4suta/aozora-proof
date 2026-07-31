use std::error::Error;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;

use aozora_proof_core::apply_safe;
use similar::TextDiff;
use tempfile::NamedTempFile;

use crate::config::Resolved;
use crate::document::Document;

#[derive(Debug)]
pub(crate) struct FixOutput {
    pub(crate) stdout: Vec<u8>,
    pub(crate) changed_files: usize,
}

#[derive(Debug)]
pub(crate) struct FixCommandError {
    message: String,
}

impl fmt::Display for FixCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for FixCommandError {}

pub(crate) fn run(
    documents: &[Document],
    settings: &Resolved,
    dry_run: bool,
) -> Result<FixOutput, FixCommandError> {
    let mut stdout = Vec::new();
    let mut changed_files = 0usize;
    for document in documents {
        if !settings.autofix_for(&document.label) {
            return Err(message(format!(
                "{}: autofix is disabled by configuration",
                document.label
            )));
        }
        let Some(orthography) = settings.orthography_for(&document.label) else {
            return Err(message(format!(
                "{}: orthography is required",
                document.label
            )));
        };
        let fixed = apply_safe(&document.raw, orthography)
            .map_err(|source| message(format!("{}: {source}", document.label)))?;
        if !fixed.changed {
            if document.path.is_none() && !dry_run {
                stdout.extend_from_slice(&fixed.bytes);
            }
            continue;
        }
        changed_files += 1;
        if dry_run {
            stdout.extend_from_slice(
                unified_diff(&document.label, &document.report.decoded, &fixed.decoded).as_bytes(),
            );
            continue;
        }
        if let Some(path) = &document.path {
            atomic_write(path, &document.raw, &fixed.bytes)
                .map_err(|source| message(format!("{}: {source}", document.label)))?;
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
    let metadata = fs::metadata(path).map_err(|source| message(source.to_string()))?;
    let parent = path
        .parent()
        .ok_or_else(|| message("target file has no parent directory"))?;
    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| message(source.to_string()))?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|source| message(source.to_string()))?;
    temporary
        .write_all(replacement)
        .map_err(|source| message(source.to_string()))?;
    temporary
        .flush()
        .map_err(|source| message(source.to_string()))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| message(source.to_string()))?;
    let current = fs::read(path).map_err(|source| message(source.to_string()))?;
    if current != original {
        return Err(message(
            "file changed after it was read; refusing to overwrite",
        ));
    }
    temporary
        .persist(path)
        .map_err(|source| message(source.error.to_string()))?;
    Ok(())
}

fn message(value: impl Into<String>) -> FixCommandError {
    FixCommandError {
        message: value.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn concurrent_change_is_rejected_without_overwrite() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("work.txt");
        fs::write(&path, b"original")?;
        fs::write(&path, b"external")?;

        assert!(atomic_write(&path, b"original", b"replacement").is_err());
        assert_eq!(fs::read(path)?, b"external");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_permissions() -> Result<(), Box<dyn Error>> {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir()?;
        let path = directory.path().join("work.txt");
        fs::write(&path, b"original")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;

        atomic_write(&path, b"original", b"replacement")?;

        assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o640);
        Ok(())
    }
}

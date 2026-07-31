use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use aozora_proof_core::{CheckError, Report, run_submission_with_orthography};

use crate::config::Resolved;
use crate::discovery::Input;

#[derive(Debug)]
pub(crate) struct Document {
    pub(crate) label: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) raw: Vec<u8>,
    pub(crate) report: Report,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DocumentError {
    #[error("{label}: could not read input: {source}")]
    Read {
        label: String,
        #[source]
        source: io::Error,
    },
    #[error("standard input could not be read: {source}")]
    Stdin {
        #[source]
        source: io::Error,
    },
    #[error("{label}: orthography is required; pass --orthography")]
    MissingOrthography { label: String },
    #[error("{label}: proofreading failed: {source}")]
    Check {
        label: String,
        #[source]
        source: CheckError,
    },
}

impl DocumentError {
    pub(crate) const fn is_internal(&self) -> bool {
        matches!(self, Self::Check { source, .. } if source.is_internal())
    }
}

pub(crate) fn load(inputs: &[Input], settings: &Resolved) -> Result<Vec<Document>, DocumentError> {
    let mut documents = Vec::with_capacity(inputs.len());
    for input in inputs {
        let raw = input.path.as_ref().map_or_else(read_stdin, |path| {
            fs::read(path).map_err(|source| DocumentError::Read {
                label: input.label.clone(),
                source,
            })
        })?;
        let Some(orthography) = settings.orthography_for(&input.label) else {
            return Err(DocumentError::MissingOrthography {
                label: input.label.clone(),
            });
        };
        let mut report = run_submission_with_orthography(&raw, orthography).map_err(|source| {
            DocumentError::Check {
                label: input.label.clone(),
                source,
            }
        })?;
        settings.apply_rule_levels(&input.label, &mut report);
        documents.push(Document {
            label: input.label.clone(),
            path: input.path.clone(),
            raw,
            report,
        });
    }
    Ok(documents)
}

fn read_stdin() -> Result<Vec<u8>, DocumentError> {
    let mut raw = Vec::new();
    io::stdin()
        .read_to_end(&mut raw)
        .map_err(|source| DocumentError::Stdin { source })?;
    Ok(raw)
}

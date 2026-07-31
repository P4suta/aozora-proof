use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use aozora_proof_core::moji::file_checks::DetectedEncoding;
use aozora_proof_core::{Report, run_submission_with_orthography};

use crate::config::Resolved;
use crate::discovery::Input;

#[derive(Debug)]
pub(crate) struct Document {
    pub(crate) label: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) raw: Vec<u8>,
    pub(crate) report: Report,
}

#[derive(Debug)]
pub(crate) struct DocumentError {
    message: String,
}

impl fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DocumentError {}

pub(crate) fn load(inputs: &[Input], settings: &Resolved) -> Result<Vec<Document>, DocumentError> {
    let mut documents = Vec::with_capacity(inputs.len());
    for input in inputs {
        let raw = input.path.as_ref().map_or_else(read_stdin, |path| {
            fs::read(path).map_err(|source| DocumentError {
                message: format!("{}: {source}", path.display()),
            })
        })?;
        let Some(orthography) = settings.orthography_for(&input.label) else {
            return Err(DocumentError {
                message: format!(
                    "{}: orthography is required; pass --orthography",
                    input.label
                ),
            });
        };
        let mut report = run_submission_with_orthography(&raw, orthography);
        if report.encoding == DetectedEncoding::Unknown {
            return Err(DocumentError {
                message: format!(
                    "{}: input cannot be decoded as UTF-8 or Shift_JIS",
                    input.label
                ),
            });
        }
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
        .map_err(|source| DocumentError {
            message: format!("standard input: {source}"),
        })?;
    Ok(raw)
}

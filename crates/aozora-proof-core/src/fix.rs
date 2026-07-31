//! Safe-fix planning and lossless Shift_JIS materialization.

use std::borrow::Cow;
use std::error::Error;
use std::fmt;

use encoding_rs::SHIFT_JIS;

use crate::finding::{FixApplicability, FixOperation, TextEdit};
use crate::orthography::Orthography;

const FIX_POINT_LIMIT: usize = 8;

/// Failure to construct one atomic safe-fix result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixError {
    /// The input cannot be decoded without replacement.
    Decode,
    /// Two safe edits target overlapping decoded ranges.
    OverlappingEdits,
    /// A text edit does not lie on valid UTF-8 boundaries.
    InvalidEdit,
    /// Safe rules did not converge.
    NonConvergent,
    /// The final text cannot round-trip through Shift_JIS.
    ShiftJisLossy,
}

impl fmt::Display for FixError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Decode => "input cannot be decoded as UTF-8 or Shift_JIS",
            Self::OverlappingEdits => "safe edits overlap",
            Self::InvalidEdit => "a safe edit has an invalid UTF-8 range",
            Self::NonConvergent => "safe fixes did not reach a fixed point",
            Self::ShiftJisLossy => "fixed text cannot round-trip through Shift_JIS",
        };
        formatter.write_str(message)
    }
}

impl Error for FixError {}

/// Fully validated result for one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeFixResult {
    /// Submission bytes to write or stream.
    pub bytes: Vec<u8>,
    /// Decoded final text.
    pub decoded: String,
    /// Whether output bytes differ from input.
    pub changed: bool,
    /// Number of text-edit passes needed to reach the fixed point.
    pub passes: usize,
}

/// Apply every safe fix in memory and produce lossless Shift_JIS bytes.
///
/// Review-only alternatives are never selected.
///
/// # Errors
///
/// Returns [`FixError`] when decoding, edit validation, convergence, or the
/// final Shift_JIS round trip fails.
pub fn apply_safe(raw: &[u8], orthography: Orthography) -> Result<SafeFixResult, FixError> {
    let without_bom = raw.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(raw);
    let decoded = aozora::decode_auto(without_bom).map_err(|_| FixError::Decode)?;
    let mut current = decoded
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&decoded)
        .to_owned();
    let mut passes = 0usize;

    loop {
        let report =
            crate::pipeline::run_submission_with_orthography(current.as_bytes(), orthography);
        let edits: Vec<TextEdit> = report
            .findings
            .iter()
            .flat_map(|finding| &finding.fixes)
            .filter(|fix| fix.applicability == FixApplicability::Safe)
            .filter_map(|fix| match &fix.operation {
                FixOperation::Text(edit) => Some(edit.clone()),
                FixOperation::RemoveBom
                | FixOperation::NormalizeCrLf
                | FixOperation::EnsureFinalNewline
                | FixOperation::EncodeShiftJis => None,
            })
            .collect();
        if edits.is_empty() {
            break;
        }
        if passes == FIX_POINT_LIMIT {
            return Err(FixError::NonConvergent);
        }
        current = apply_text_edits(&current, &edits)?;
        passes += 1;
    }

    let normalized = normalize_submission_text(&current);
    let (encoded, _, had_errors) = SHIFT_JIS.encode(&normalized);
    if had_errors {
        return Err(FixError::ShiftJisLossy);
    }
    let bytes = encoded.into_owned();
    let (round_trip, decode_errors) = SHIFT_JIS.decode_without_bom_handling(&bytes);
    if decode_errors || round_trip != Cow::Borrowed(normalized.as_str()) {
        return Err(FixError::ShiftJisLossy);
    }
    let verification = crate::pipeline::run_submission_with_orthography(&bytes, orthography);
    if verification
        .findings
        .iter()
        .flat_map(|finding| &finding.fixes)
        .any(|fix| fix.applicability == FixApplicability::Safe)
    {
        return Err(FixError::NonConvergent);
    }
    Ok(SafeFixResult {
        changed: bytes != raw,
        bytes,
        decoded: normalized,
        passes,
    })
}

/// Apply decoded text edits after rejecting overlap and invalid boundaries.
///
/// # Errors
///
/// Returns [`FixError::OverlappingEdits`] or [`FixError::InvalidEdit`] without
/// producing partial output.
pub fn apply_text_edits(source: &str, edits: &[TextEdit]) -> Result<String, FixError> {
    let mut ordered = edits.to_vec();
    ordered.sort_by_key(|edit| (edit.span.start, edit.span.end));
    if ordered.windows(2).any(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(left, right)| left.span.end > right.span.start)
    }) {
        return Err(FixError::OverlappingEdits);
    }

    let mut output = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for edit in &ordered {
        let start = usize::try_from(edit.span.start).map_err(|_| FixError::InvalidEdit)?;
        let end = usize::try_from(edit.span.end).map_err(|_| FixError::InvalidEdit)?;
        let unchanged = source.get(cursor..start).ok_or(FixError::InvalidEdit)?;
        source.get(start..end).ok_or(FixError::InvalidEdit)?;
        output.push_str(unchanged);
        output.push_str(&edit.replacement);
        cursor = end;
    }
    output.push_str(source.get(cursor..).ok_or(FixError::InvalidEdit)?);
    Ok(output)
}

fn normalize_submission_text(source: &str) -> String {
    let mut lf = String::with_capacity(source.len() + 1);
    let mut characters = source.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                lf.push('\n');
            }
            '\n' => lf.push('\n'),
            _ => lf.push(character),
        }
    }
    if !lf.is_empty() && !lf.ends_with('\n') {
        lf.push('\n');
    }
    let mut crlf = String::with_capacity(lf.len() + lf.lines().count());
    for character in lf.chars() {
        if character == '\n' {
            crlf.push_str("\r\n");
        } else {
            crlf.push(character);
        }
    }
    crlf
}

#[cfg(test)]
mod tests {
    use crate::finding::Span;
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn safe_fix_is_idempotent_and_composes_halfwidth_kana() {
        let first = apply_safe("ｶﾞ\n".as_bytes(), Orthography::Mixed).expect("first fix");
        assert!(first.changed);
        assert_eq!(first.decoded, "ガ\r\n");
        let second = apply_safe(&first.bytes, Orthography::Mixed).expect("second fix");
        assert!(!second.changed);
        assert_eq!(first.bytes, second.bytes);
    }

    #[test]
    fn overlapping_edits_are_rejected() {
        let edits = [
            TextEdit {
                span: Span { start: 0, end: 3 },
                replacement: "a".to_owned(),
            },
            TextEdit {
                span: Span { start: 2, end: 4 },
                replacement: "b".to_owned(),
            },
        ];
        assert_eq!(
            apply_text_edits("青空", &edits),
            Err(FixError::OverlappingEdits)
        );
    }

    #[test]
    fn unrepresentable_text_aborts_the_file() {
        assert_eq!(
            apply_safe("🍣".as_bytes(), Orthography::Mixed),
            Err(FixError::ShiftJisLossy)
        );
    }

    proptest! {
        #[test]
        fn safe_fix_is_idempotent_for_ascii_documents(source in "[ -~\\r\\n]{0,256}") {
            let first = apply_safe(source.as_bytes(), Orthography::Mixed)
                .expect("ASCII input is representable");
            let second = apply_safe(&first.bytes, Orthography::Mixed)
                .expect("fixed output remains representable");
            prop_assert!(!second.changed);
            prop_assert_eq!(first.bytes, second.bytes);
        }
    }
}

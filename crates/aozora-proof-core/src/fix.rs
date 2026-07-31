//! Safe-fix planning and lossless Shift_JIS materialization.

use encoding_rs::SHIFT_JIS;
use std::borrow::Cow;

use crate::CheckError;
use crate::finding::{FixApplicability, FixOperation, TextEdit};
use crate::orthography::Orthography;

const FIX_POINT_LIMIT: usize = 8;

/// Failure to construct one atomic safe-fix result.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FixError {
    /// The input cannot be decoded without replacement.
    #[error("input cannot be decoded as UTF-8 or Shift_JIS")]
    Decode {
        /// Upstream decoder failure.
        #[source]
        source: aozora::DecodeError,
    },
    /// The proofreading pipeline could not produce a complete report.
    #[error("proofreading failed while planning fixes")]
    Check {
        /// Checked pipeline failure.
        #[source]
        source: CheckError,
    },
    /// Two safe edits target overlapping decoded ranges.
    #[error("safe edits overlap")]
    OverlappingEdits,
    /// A text edit does not lie on valid UTF-8 boundaries.
    #[error("a safe edit has an invalid UTF-8 range")]
    InvalidEdit,
    /// A wire edit offset cannot be represented on the host.
    #[error("safe edit offset cannot be represented on this host")]
    EditOffset {
        /// Failed integer conversion.
        #[source]
        source: std::num::TryFromIntError,
    },
    /// Safe rules did not converge.
    #[error("safe fixes did not reach a fixed point")]
    NonConvergent,
    /// The final text cannot round-trip through Shift_JIS.
    #[error("fixed text cannot round-trip through Shift_JIS")]
    ShiftJisLossy,
    /// Pass accounting exceeded the host coordinate type.
    #[error("safe-fix pass accounting overflowed")]
    PassOverflow,
}

impl FixError {
    /// Whether the failure represents a violated engine invariant.
    #[must_use]
    pub const fn is_internal(&self) -> bool {
        match self {
            Self::Check { source } => source.is_internal(),
            Self::OverlappingEdits
            | Self::InvalidEdit
            | Self::EditOffset { .. }
            | Self::NonConvergent
            | Self::PassOverflow => true,
            Self::Decode { .. } | Self::ShiftJisLossy => false,
        }
    }
}

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
    let decoded = aozora::decode_auto(without_bom).map_err(|source| FixError::Decode { source })?;
    let mut current = decoded
        .strip_prefix('\u{FEFF}')
        .unwrap_or(&decoded)
        .to_owned();
    let mut passes = 0usize;

    loop {
        let report =
            crate::pipeline::run_submission_with_orthography(current.as_bytes(), orthography)
                .map_err(|source| FixError::Check { source })?;
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
        passes = passes.checked_add(1).ok_or(FixError::PassOverflow)?;
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
    let verification = crate::pipeline::run_submission_with_orthography(&bytes, orthography)
        .map_err(|source| FixError::Check { source })?;
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
        let start =
            usize::try_from(edit.span.start).map_err(|source| FixError::EditOffset { source })?;
        let end =
            usize::try_from(edit.span.end).map_err(|source| FixError::EditOffset { source })?;
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
    let mut lf = String::with_capacity(source.len().saturating_add(1));
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
    let mut crlf = String::with_capacity(lf.len().saturating_add(lf.lines().count()));
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
        assert!(matches!(
            apply_text_edits("青空", &edits),
            Err(FixError::OverlappingEdits)
        ));
    }

    #[test]
    fn unrepresentable_text_aborts_the_file() {
        assert!(matches!(
            apply_safe("🍣".as_bytes(), Orthography::Mixed),
            Err(FixError::ShiftJisLossy)
        ));
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

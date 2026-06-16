//! Coordinate-frame translation — the engine's highest-risk primitive.
//!
//! Three byte-offset frames are in play:
//!
//! - **raw** — the original file bytes (BOM, CRLF, possibly `Shift_JIS`).
//! - **decoded** — the UTF-8 string the caller hands to the parser. This is
//!   the engine's unified reporting frame; character checks run here.
//! - **sanitized** — `aozora`'s Phase-0 output (leading BOM stripped,
//!   CRLF→LF folded, `〔…〕` accent ranges decomposed). The parser reports
//!   diagnostic spans in *this* frame.
//!
//! To merge parser diagnostics with character findings we must lift each
//! parser span from **sanitized** back to **decoded**. [`SpanMap`] does that
//! by aligning the sanitized text (which `aozora` derives from the decoded
//! input) against the decoded input it came from.
//!
//! The map is **exact** for the deletion-based transforms (BOM strip,
//! CRLF→LF). The `〔…〕` accent rewrite is a *substitution*, not a deletion;
//! around such ranges the map degrades to a clamped best-effort offset.

use aozora::pipeline::lexer::sanitize;

use crate::finding::Span;

/// A sanitized→decoded byte-offset map for one document.
#[derive(Debug, Clone)]
pub struct SpanMap {
    /// `decoded_of_sanitized[s]` is the decoded byte offset corresponding to
    /// sanitized byte offset `s`. Length is `sanitized.len() + 1` (the final
    /// entry is the end sentinel). All entries are `≤ decoded.len()` and
    /// monotonically non-decreasing.
    decoded_of_sanitized: Vec<u32>,
}

impl SpanMap {
    /// Build the map for `decoded` by running `aozora`'s Phase-0 sanitize and
    /// aligning its output against `decoded`.
    #[must_use]
    pub fn build(decoded: &str) -> Self {
        let sanitized = sanitize(decoded);
        let san = sanitized.text.as_bytes();
        let dec = decoded.as_bytes();

        let mut decoded_of_sanitized = Vec::with_capacity(san.len() + 1);
        let mut d = 0usize;
        for &sb in san {
            // Skip decoded bytes that sanitize deleted (BOM, CR) before this
            // sanitized byte. For a pure-deletion transform the bytes realign
            // exactly; for a substitution this clamps at the decoded end.
            while dec.get(d).is_some_and(|&b| b != sb) {
                d += 1;
            }
            let pos = d.min(dec.len());
            decoded_of_sanitized.push(u32::try_from(pos).unwrap_or(u32::MAX));
            d = d.saturating_add(1);
        }
        let end = d.min(dec.len());
        decoded_of_sanitized.push(u32::try_from(end).unwrap_or(u32::MAX));

        Self {
            decoded_of_sanitized,
        }
    }

    /// Map a single sanitized byte offset to a decoded byte offset.
    /// Offsets beyond the sanitized text clamp to the end sentinel.
    #[must_use]
    pub fn offset(&self, sanitized: u32) -> u32 {
        let i = usize::try_from(sanitized).unwrap_or(usize::MAX);
        self.decoded_of_sanitized
            .get(i)
            .copied()
            .unwrap_or_else(|| self.decoded_of_sanitized.last().copied().unwrap_or(0))
    }

    /// Lift a sanitized-frame `aozora` span into a decoded-frame [`Span`].
    #[must_use]
    pub fn map(&self, span: aozora::Span) -> Span {
        Span {
            start: self.offset(span.start),
            end: self.offset(span.end),
        }
    }
}

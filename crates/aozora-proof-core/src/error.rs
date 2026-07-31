//! Checked failure modes for the proofreading pipeline.

/// Failure to produce a complete proofreading report.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CheckError {
    /// The input is neither valid UTF-8 nor valid Shift_JIS.
    #[error("input cannot be decoded as UTF-8 or Shift_JIS")]
    Decode {
        /// Upstream decoder failure.
        #[source]
        source: aozora::DecodeError,
    },
    /// The decoded source exceeds the coordinate representation.
    #[error("decoded source is {len} bytes and exceeds the u32 span limit")]
    SourceTooLarge {
        /// Rejected byte length.
        len: usize,
    },
    /// The upstream notation parser rejected the decoded source.
    #[error("notation parser rejected the decoded source")]
    Parse {
        /// Upstream parser failure.
        #[source]
        source: aozora::ParseError,
    },
    /// A proofreader-owned rule code is absent from the catalog.
    #[error("rule code {code} is absent from the catalog")]
    UnknownRule {
        /// Unrecognized stable code.
        code: &'static str,
    },
    /// A decoded byte range cannot be represented as a wire span.
    #[error("decoded byte span {start}..{end} exceeds the u32 coordinate limit")]
    SpanOverflow {
        /// Inclusive start offset.
        start: usize,
        /// Exclusive end offset.
        end: usize,
        /// Failed integer conversion.
        #[source]
        source: std::num::TryFromIntError,
    },
    /// A span is inverted, out of bounds, or not on UTF-8 boundaries.
    #[error("decoded byte span {start}..{end} is invalid for a {source_len}-byte source")]
    InvalidSpan {
        /// Inclusive start offset.
        start: u32,
        /// Exclusive end offset.
        end: u32,
        /// Decoded source length.
        source_len: usize,
    },
    /// A wire offset cannot be represented by the host coordinate type.
    #[error("wire offset {byte} cannot be represented by the host coordinate type")]
    CoordinateConversion {
        /// Rejected wire offset.
        byte: u32,
        /// Failed integer conversion.
        #[source]
        source: std::num::TryFromIntError,
    },
    /// Checked coordinate arithmetic could not represent its result.
    #[error("coordinate arithmetic overflow while {operation}")]
    CoordinateOverflow {
        /// Operation that could not be represented.
        operation: &'static str,
    },
    /// An iterator invariant required by a detector was violated.
    #[error("detector invariant failed while {operation}")]
    DetectorInvariant {
        /// Detector operation that failed.
        operation: &'static str,
    },
}

impl CheckError {
    /// Whether the failure represents a violated engine invariant.
    #[must_use]
    pub const fn is_internal(&self) -> bool {
        matches!(
            self,
            Self::UnknownRule { .. }
                | Self::SpanOverflow { .. }
                | Self::InvalidSpan { .. }
                | Self::CoordinateConversion { .. }
                | Self::CoordinateOverflow { .. }
                | Self::DetectorInvariant { .. }
        )
    }
}

//! Directional orthography policy for document commands.

/// Character-form policy selected for a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orthography {
    /// Review traditional forms that have a modern counterpart.
    Modern,
    /// Review modern forms and every recorded traditional counterpart.
    Traditional,
    /// Do not infer a direction.
    Mixed,
}

impl Orthography {
    /// Canonical configuration and wire identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Traditional => "traditional",
            Self::Mixed => "mixed",
        }
    }
}

//! Finding, coordinate, and structured-fix types shared by every frontend.

use std::collections::BTreeMap;

/// Machine-report schema version.
pub const SCHEMA_VERSION: u32 = 2;

/// Finding severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A submission defect that can fail the configured gate.
    Error,
    /// A likely problem that should be resolved or explicitly retained.
    Warning,
    /// Advisory information.
    Note,
}

impl Severity {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// Stable rule category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleCategory {
    /// File encoding, byte order marks, and line endings.
    Encoding,
    /// Character repertoire and control characters.
    Character,
    /// Aozora notation structure.
    Notation,
    /// Modern and traditional character-form policy.
    Orthography,
    /// Ruby boundaries and grouping.
    Ruby,
    /// Spacing, punctuation, and source layout.
    Layout,
    /// Opening legend and closing bibliography.
    Bibliography,
    /// Checks that require the base edition or editorial judgement.
    Manual,
}

impl RuleCategory {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Encoding => "encoding",
            Self::Character => "character",
            Self::Notation => "notation",
            Self::Orthography => "orthography",
            Self::Ruby => "ruby",
            Self::Layout => "layout",
            Self::Bibliography => "bibliography",
            Self::Manual => "manual",
        }
    }
}

/// How a rule can be evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionClass {
    /// The engine can determine the result without editorial judgement.
    Automatic,
    /// The engine can identify a candidate but a person decides it.
    Review,
    /// The requirement cannot be established from the text alone.
    Manual,
}

impl DetectionClass {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Review => "review",
            Self::Manual => "manual",
        }
    }
}

/// Which engine layer produced a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// An upstream parser diagnostic.
    Notation,
    /// Character or file conformance.
    Character,
    /// Orthography policy.
    Orthography,
    /// Gaiji reference policy.
    Gaiji,
    /// Submission structure.
    Submission,
}

impl Origin {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Notation => "notation",
            Self::Character => "character",
            Self::Orthography => "orthography",
            Self::Gaiji => "gaiji",
            Self::Submission => "submission",
        }
    }
}

/// Whether a finding traces to source text or an engine invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSource {
    /// The input document.
    Source,
    /// An invariant violation in the tool.
    Internal,
}

impl FindingSource {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Internal => "internal",
        }
    }
}

/// Half-open UTF-8 byte range in the decoded source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    /// Inclusive start.
    pub start: u32,
    /// Exclusive end.
    pub end: u32,
}

impl From<aozora::Span> for Span {
    fn from(value: aozora::Span) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

/// One-based Unicode code-point position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// One-based line.
    pub line: usize,
    /// One-based column measured in Unicode code points.
    pub column: usize,
}

/// Whether an offered fix may be applied unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixApplicability {
    /// One meaning-independent replacement is known.
    Safe,
    /// A person must inspect the source and choose.
    Review,
}

impl FixApplicability {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Review => "review",
        }
    }
}

/// A decoded-source text replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Range to replace.
    pub span: Span,
    /// Replacement text.
    pub replacement: String,
}

/// A transformation represented by a fix alternative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixOperation {
    /// Replace decoded source text.
    Text(TextEdit),
    /// Remove a leading byte-order mark.
    RemoveBom,
    /// Convert every line ending to CRLF.
    NormalizeCrLf,
    /// Add the required final line ending.
    EnsureFinalNewline,
    /// Encode the final document as Shift_JIS.
    EncodeShiftJis,
}

impl FixOperation {
    /// Canonical machine identifier.
    #[must_use]
    pub const fn as_wire_str(&self) -> &'static str {
        match self {
            Self::Text(_) => "replaceText",
            Self::RemoveBom => "removeBom",
            Self::NormalizeCrLf => "normalizeCrLf",
            Self::EnsureFinalNewline => "ensureFinalNewline",
            Self::EncodeShiftJis => "encodeShiftJis",
        }
    }
}

/// One possible correction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixAlternative {
    /// Applicability classification.
    pub applicability: FixApplicability,
    /// Canonical English label.
    pub label: String,
    /// Japanese human-interface label.
    pub label_ja: String,
    /// Transformation to stage.
    pub operation: FixOperation,
}

impl FixAlternative {
    /// Construct a safe text replacement.
    #[must_use]
    pub const fn safe_text(
        span: Span,
        replacement: String,
        label: String,
        label_ja: String,
    ) -> Self {
        Self {
            applicability: FixApplicability::Safe,
            label,
            label_ja,
            operation: FixOperation::Text(TextEdit { span, replacement }),
        }
    }

    /// Construct a review-only text replacement.
    #[must_use]
    pub const fn review_text(
        span: Span,
        replacement: String,
        label: String,
        label_ja: String,
    ) -> Self {
        Self {
            applicability: FixApplicability::Review,
            label,
            label_ja,
            operation: FixOperation::Text(TextEdit { span, replacement }),
        }
    }
}

/// Dynamic fields used to construct a catalog-backed finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingDetails {
    message: String,
    message_ja: String,
    data: BTreeMap<String, String>,
    codepoint: Option<char>,
    fixes: Vec<FixAlternative>,
}

impl FindingDetails {
    /// Construct localized messages with no additional context or fixes.
    #[must_use]
    pub const fn new(message: String, message_ja: String) -> Self {
        Self {
            message,
            message_ja,
            data: BTreeMap::new(),
            codepoint: None,
            fixes: Vec::new(),
        }
    }

    /// Attach stable structured context.
    #[must_use]
    pub fn with_data(mut self, data: BTreeMap<String, String>) -> Self {
        self.data = data;
        self
    }

    /// Attach the offending scalar.
    #[must_use]
    pub const fn with_codepoint(mut self, codepoint: char) -> Self {
        self.codepoint = Some(codepoint);
        self
    }

    /// Attach correction alternatives.
    #[must_use]
    pub fn with_fixes(mut self, fixes: Vec<FixAlternative>) -> Self {
        self.fixes = fixes;
        self
    }
}

/// A single proofreading result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Stable rule code.
    pub code: &'static str,
    /// Catalog category.
    pub category: RuleCategory,
    /// Severity after configuration.
    pub severity: Severity,
    /// Producing engine layer.
    pub origin: Origin,
    /// Source or internal invariant.
    pub source: FindingSource,
    /// Decoded UTF-8 byte range.
    pub span: Span,
    /// Canonical English message.
    pub message: String,
    /// Japanese human-interface message.
    pub message_ja: String,
    /// Stable structured context.
    pub data: BTreeMap<String, String>,
    /// Official or upstream authority.
    pub authority_url: &'static str,
    /// Offending scalar when the rule is character-based.
    pub codepoint: Option<char>,
    /// Available corrections.
    pub fixes: Vec<FixAlternative>,
}

impl Finding {
    /// Construct a proofreader-owned finding from catalog metadata.
    #[must_use]
    pub fn from_rule(
        code: &'static str,
        origin: Origin,
        span: Span,
        details: FindingDetails,
    ) -> Self {
        let rule = crate::rules::explain(code).unwrap_or_else(crate::rules::upstream_rule);
        debug_assert_eq!(rule.code, code);
        Self {
            code,
            category: rule.category,
            severity: rule.default_severity,
            origin,
            source: FindingSource::Source,
            span,
            message: details.message,
            message_ja: details.message_ja,
            data: details.data,
            authority_url: rule.authority_url,
            codepoint: details.codepoint,
            fixes: details.fixes,
        }
    }

    /// Trailing code token used by SARIF rule names.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.code.rsplit("::").next().unwrap_or(self.code)
    }

    /// Localized message for a human renderer.
    #[must_use]
    pub fn localized_message(&self, japanese: bool) -> &str {
        if japanese {
            &self.message_ja
        } else {
            &self.message
        }
    }

    /// One-based position for this finding.
    #[must_use]
    pub fn position(&self, decoded: &str) -> Position {
        position(decoded, self.span.start)
    }
}

/// Convert a decoded UTF-8 byte offset to a one-based Unicode position.
#[must_use]
pub fn position(text: &str, byte: u32) -> Position {
    let limit = usize::try_from(byte).unwrap_or(usize::MAX);
    let mut line = 1usize;
    let mut column = 1usize;
    for (offset, character) in text.char_indices() {
        if offset >= limit {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    Position { line, column }
}

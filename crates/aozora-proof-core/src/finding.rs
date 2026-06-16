//! The unified finding model and its JSON wire projection.
//!
//! # Wire format
//!
//! Findings serialise to the same `{ "schema_version", "data" }` envelope
//! shape the `aozora` parser uses, and each entry is a **strict superset**
//! of `aozora`'s diagnostic wire shape (`kind` / `severity` / `source` /
//! `span`). A tool that already reads `aozora` wire therefore reads the
//! notation subset of our output unchanged; the extra fields (`code`,
//! `origin`, `suggestion`) are additive.
//!
//! [`SCHEMA_VERSION`] is owned by this crate (the proofreader's wire is a
//! superset, so it versions independently of the parser's schema).
//!
//! # Code namespaces
//!
//! Character-level findings carry stable, namespaced codes that parallel
//! `aozora`'s `aozora::lex::*`:
//!
//! - `aozora::char::*`  — character conformance (JIS, 機種依存, width, file)
//! - `aozora::kyuji::*` — old-/new-form kanji (旧字体↔新字体)
//! - `aozora::gaiji::*` — gaiji (外字) lookup surface
//!
//! Notation findings reuse the parser's own `aozora::lex::*` codes verbatim.

use serde::Serialize;

/// Wire-format schema version for the proofreader's `Report` envelope.
/// Bumped on any breaking change to the serialised shape.
pub const SCHEMA_VERSION: u32 = 1;

/// Severity of a [`Finding`].
///
/// Mirrors `aozora::Severity` but is owned here so the public API does not
/// leak the parser's `#[non_exhaustive]` enum into our surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A genuine defect; fails CI under `--fail-on error`.
    Error,
    /// A recoverable issue the author should see.
    Warning,
    /// Informational; never affects CI status on its own.
    Note,
}

impl Severity {
    /// Stable lowercase wire identifier (`"error"` / `"warning"` / `"note"`).
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// Which sub-tool produced a [`Finding`] — used to group output by tab and
/// to route per-tool CLI subcommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// From the `aozora` parser's notation diagnostics.
    Notation,
    /// Character-level conformance (JIS / 機種依存 / width / file).
    Character,
    /// Old-/new-form kanji (旧字体↔新字体).
    Kyuji,
    /// Gaiji (外字) lookup surface.
    Gaiji,
}

impl Origin {
    /// Stable lowercase wire identifier.
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Notation => "notation",
            Self::Character => "character",
            Self::Kyuji => "kyuji",
            Self::Gaiji => "gaiji",
        }
    }
}

/// Whether a [`Finding`] traces to user input or to a library-internal bug.
///
/// Mirrors `aozora::DiagnosticSource`. An `Internal` finding maps to CLI
/// exit code `3` — "the tool is wrong", as distinct from "the file is
/// wrong" (exit `1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingSource {
    /// Traces to the user-provided source text.
    Source,
    /// A pipeline-internal invariant violation (a bug to report upstream).
    Internal,
}

impl FindingSource {
    /// Stable lowercase wire identifier (`"source"` / `"internal"`).
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Internal => "internal",
        }
    }
}

/// A byte span in the unified DECODED coordinate frame. Matches `aozora`'s
/// `Span` shape on the wire (`{ start, end }`, `u32`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Start byte offset (inclusive).
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

impl From<aozora::Span> for Span {
    fn from(s: aozora::Span) -> Self {
        Self {
            start: s.start,
            end: s.end,
        }
    }
}

/// A suggested fix: an 旧字体/新字体 replacement, or an "insert this 外字注記"
/// autofix for a character that needs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// Replacement text to substitute over [`Self::span`].
    pub replacement: String,
    /// The exact decoded range the replacement applies to.
    pub span: Span,
    /// Human-readable label, e.g. `旧字体「廣」→ 新字体「広」`.
    pub label: String,
}

/// A single proofreading finding, in the unified DECODED coordinate frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// Stable, namespaced code (e.g. `aozora::char::needs_gaiji_chuki`, or
    /// a reused `aozora::lex::*` code for notation findings).
    pub code: &'static str,
    /// Severity.
    pub severity: Severity,
    /// Producing sub-tool.
    pub origin: Origin,
    /// User-input vs library-internal.
    pub source: FindingSource,
    /// Location, decoded-string byte offsets.
    pub span: Span,
    /// Human-readable message (Japanese).
    pub message: String,
    /// The offending character, where the finding is about one char.
    pub codepoint: Option<char>,
    /// A suggested fix, where one exists.
    pub suggestion: Option<Suggestion>,
}

impl Finding {
    /// The wire `kind`: the trailing token of [`Self::code`] after the last
    /// `::`. Matches `aozora`'s convention of emitting the short tag in the
    /// `kind` field while the fully-qualified code travels in `code`.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.code.rsplit("::").next().unwrap_or(self.code)
    }
}

// ---- wire projection -------------------------------------------------

#[derive(Serialize)]
struct SpanWire {
    start: u32,
    end: u32,
}

impl From<Span> for SpanWire {
    fn from(s: Span) -> Self {
        Self {
            start: s.start,
            end: s.end,
        }
    }
}

#[derive(Serialize)]
struct SuggestionWire<'a> {
    replacement: &'a str,
    span: SpanWire,
    label: &'a str,
}

#[derive(Serialize)]
struct FindingWire<'a> {
    code: &'a str,
    kind: &'a str,
    severity: &'a str,
    origin: &'a str,
    source: &'a str,
    span: SpanWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    codepoint: Option<char>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggestion: Option<SuggestionWire<'a>>,
}

impl<'a> From<&'a Finding> for FindingWire<'a> {
    fn from(f: &'a Finding) -> Self {
        Self {
            code: f.code,
            kind: f.kind(),
            severity: f.severity.as_wire_str(),
            origin: f.origin.as_wire_str(),
            source: f.source.as_wire_str(),
            span: f.span.into(),
            codepoint: f.codepoint,
            suggestion: f.suggestion.as_ref().map(|s| SuggestionWire {
                replacement: &s.replacement,
                span: s.span.into(),
                label: &s.label,
            }),
        }
    }
}

/// A [`Finding`] serialises directly to its wire shape (a superset of
/// `aozora`'s diagnostic wire entry), so callers can embed it in their own
/// envelopes (e.g. a per-file `{ path, data }`).
impl Serialize for Finding {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FindingWire::from(self).serialize(serializer)
    }
}

#[derive(Serialize)]
struct Envelope<'a> {
    schema_version: u32,
    data: Vec<FindingWire<'a>>,
}

/// Project a slice of [`Finding`] into the `{ schema_version, data }` JSON
/// envelope. Empty input yields `{"schema_version":1,"data":[]}`.
#[must_use]
pub fn serialize_findings(findings: &[Finding]) -> String {
    let data = findings.iter().map(FindingWire::from).collect();
    let envelope = Envelope {
        schema_version: SCHEMA_VERSION,
        data,
    };
    serde_json::to_string(&envelope)
        .unwrap_or_else(|_| String::from(r#"{"schema_version":1,"data":[]}"#))
}

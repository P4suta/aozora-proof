//! Unpublished development commands for corpus validation and repository policy.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aozora_proof_core::{CheckError, FindingSource, Span, run_submission};
use serde::Serialize;
use syn::visit::Visit;

const AUDIT_SCHEMA_VERSION: u32 = 2;
const DEFAULT_SAMPLES: usize = 5;

#[derive(Debug)]
enum Command {
    Audit(AuditArgs),
    RustPolicy,
}

#[derive(Debug)]
struct AuditArgs {
    corpus: PathBuf,
    output: PathBuf,
    samples: usize,
}

#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("{message}")]
    Usage { message: String },
    #[error("argument {argument:?} must be UTF-8")]
    NonUtf8Argument { argument: std::ffi::OsString },
    #[error("invalid sample count {value:?}")]
    SampleCount {
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("{}: could not {operation}", path.display())]
    Io {
        path: PathBuf,
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{} is not a directory", path.display())]
    NotDirectory { path: PathBuf },
    #[error("{}: proofreading failed", path.display())]
    Check {
        path: PathBuf,
        #[source]
        source: CheckError,
    },
    #[error("could not serialize the corpus audit")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("{}: could not parse repository-owned Rust", path.display())]
    RustSyntax {
        path: PathBuf,
        #[source]
        source: syn::Error,
    },
    #[error("trait objects are forbidden in repository-owned Rust: {paths:?}")]
    TraitObjects { paths: Vec<PathBuf> },
}

#[derive(Debug, Serialize)]
struct Audit {
    schema_version: u32,
    corpus: String,
    files_scanned: u64,
    bytes_scanned: u64,
    files_with_findings: u64,
    internal_findings: u64,
    rules: BTreeMap<String, RuleStats>,
    experimental_rules: BTreeMap<String, RuleStats>,
}

#[derive(Debug, Default, Serialize)]
struct RuleStats {
    severity: String,
    findings: u64,
    files: u64,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    codepoints: BTreeMap<String, u64>,
    samples: Vec<String>,
}

fn main() -> ExitCode {
    match parse_args().and_then(run) {
        Ok(Some(output)) => {
            eprintln!("corpus audit: {}", output.display());
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::from(2)
        }
    }
}

fn parse_args() -> Result<Command, XtaskError> {
    let mut args = env::args_os().skip(1);
    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("audit") => parse_audit_args(args),
        Some("lint") => {
            if args.next().as_deref() != Some(std::ffi::OsStr::new("rust-policy"))
                || args.next().is_some()
            {
                return Err(usage("usage: cargo xtask lint rust-policy"));
            }
            Ok(Command::RustPolicy)
        }
        _ => Err(usage(
            "usage: cargo xtask audit --corpus ROOT [--out FILE] [--samples N]\n       cargo xtask lint rust-policy",
        )),
    }
}

fn parse_audit_args(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Command, XtaskError> {
    let mut corpus = None;
    let mut output = PathBuf::from("target/corpus-audit.json");
    let mut samples = DEFAULT_SAMPLES;
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--corpus") => {
                corpus = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| usage("--corpus requires a path"))?,
                ));
            }
            Some("--out") => {
                output = PathBuf::from(args.next().ok_or_else(|| usage("--out requires a path"))?);
            }
            Some("--samples") => {
                let value = args
                    .next()
                    .ok_or_else(|| usage("--samples requires a number"))?;
                let value = value
                    .into_string()
                    .map_err(|argument| XtaskError::NonUtf8Argument { argument })?;
                samples = value
                    .parse()
                    .map_err(|source| XtaskError::SampleCount { value, source })?;
            }
            Some(other) => return Err(usage(format!("unknown argument: {other}"))),
            None => {
                return Err(XtaskError::NonUtf8Argument { argument: flag });
            }
        }
    }
    Ok(Command::Audit(AuditArgs {
        corpus: corpus.ok_or_else(|| usage("--corpus is required"))?,
        output,
        samples,
    }))
}

fn run(command: Command) -> Result<Option<PathBuf>, XtaskError> {
    match command {
        Command::Audit(args) => run_audit(args).map(Some),
        Command::RustPolicy => {
            let root = env::current_dir().map_err(|source| XtaskError::Io {
                path: PathBuf::from("."),
                operation: "resolve the repository root",
                source,
            })?;
            lint_rust_policy(&root)?;
            Ok(None)
        }
    }
}

fn run_audit(args: AuditArgs) -> Result<PathBuf, XtaskError> {
    let root = args
        .corpus
        .canonicalize()
        .map_err(|source| XtaskError::Io {
            path: args.corpus.clone(),
            operation: "canonicalize the corpus path",
            source,
        })?;
    if !root.is_dir() {
        return Err(XtaskError::NotDirectory { path: root });
    }

    let mut paths = Vec::new();
    collect_text_files(&root, &mut paths)?;
    paths.sort();

    let corpus = root
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map_or(".", |name| name)
        .to_owned();
    let mut audit = Audit {
        schema_version: AUDIT_SCHEMA_VERSION,
        corpus,
        files_scanned: 0,
        bytes_scanned: 0,
        files_with_findings: 0,
        internal_findings: 0,
        rules: BTreeMap::new(),
        experimental_rules: BTreeMap::new(),
    };

    for path in paths {
        audit_file(&mut audit, &root, &path, args.samples)?;
    }

    if let Some(parent) = args
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| XtaskError::Io {
            path: parent.to_path_buf(),
            operation: "create the audit output directory",
            source,
        })?;
    }
    let json =
        serde_json::to_vec_pretty(&audit).map_err(|source| XtaskError::Serialize { source })?;
    fs::write(&args.output, json).map_err(|source| XtaskError::Io {
        path: args.output.clone(),
        operation: "write the corpus audit",
        source,
    })?;
    Ok(args.output)
}

fn audit_file(
    audit: &mut Audit,
    root: &Path,
    path: &Path,
    sample_limit: usize,
) -> Result<(), XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::Io {
        path: path.to_path_buf(),
        operation: "read a corpus file",
        source,
    })?;
    let relative = relative_path(root, path);
    let report = run_submission(&bytes).map_err(|source| XtaskError::Check {
        path: path.to_path_buf(),
        source,
    })?;
    audit.files_scanned = audit.files_scanned.saturating_add(1);
    audit.bytes_scanned = audit
        .bytes_scanned
        .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
    if !report.findings.is_empty() {
        audit.files_with_findings = audit.files_with_findings.saturating_add(1);
    }

    let mut file_codes = BTreeSet::new();
    for finding in report.findings {
        if matches!(finding.source, FindingSource::Internal) {
            audit.internal_findings = audit.internal_findings.saturating_add(1);
        }
        let stats = audit.rules.entry(finding.code.to_owned()).or_default();
        finding
            .severity
            .as_wire_str()
            .clone_into(&mut stats.severity);
        stats.findings = stats.findings.saturating_add(1);
        record_codepoint(stats, finding.codepoint);
        if file_codes.insert(finding.code) {
            stats.files = stats.files.saturating_add(1);
            if stats.samples.len() < sample_limit {
                stats.samples.push(relative.clone());
            }
        }
    }

    let mut experimental_file_codes = BTreeSet::new();
    for code in experimental_findings(&report.decoded).map_err(|source| XtaskError::Check {
        path: path.to_path_buf(),
        source,
    })? {
        let stats = audit.experimental_rules.entry(code.to_owned()).or_default();
        "experimental".clone_into(&mut stats.severity);
        stats.findings = stats.findings.saturating_add(1);
        if experimental_file_codes.insert(code) {
            stats.files = stats.files.saturating_add(1);
            if stats.samples.len() < sample_limit {
                stats.samples.push(relative.clone());
            }
        }
    }
    Ok(())
}

fn usage(message: impl Into<String>) -> XtaskError {
    XtaskError::Usage {
        message: message.into(),
    }
}

fn codepoint_key(character: char) -> String {
    format!("U+{:04X}", u32::from(character))
}

fn record_codepoint(stats: &mut RuleStats, codepoint: Option<char>) {
    if let Some(character) = codepoint {
        stats
            .codepoints
            .entry(codepoint_key(character))
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
}

fn collect_text_files(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    let mut entries = read_directory(directory, "read a corpus directory")?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| XtaskError::Io {
            path: path.clone(),
            operation: "read a corpus entry type",
            source,
        })?;
        if file_type.is_dir() {
            collect_text_files(&path, paths)?;
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("txt"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn read_directory(
    directory: &Path,
    operation: &'static str,
) -> Result<Vec<fs::DirEntry>, XtaskError> {
    fs::read_dir(directory)
        .map_err(|source| XtaskError::Io {
            path: directory.to_path_buf(),
            operation,
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| XtaskError::Io {
            path: directory.to_path_buf(),
            operation,
            source,
        })
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map_or(path, |relative| relative)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

const EXPERIMENTAL_HALFWIDTH_SPACE: &str = "experimental::halfwidth_space";
const EXPERIMENTAL_ASCII_PARENTHESIS: &str = "experimental::ascii_parenthesis";
const EXPERIMENTAL_FULLWIDTH_TILDE: &str = "experimental::fullwidth_tilde";

fn experimental_findings(text: &str) -> Result<Vec<&'static str>, CheckError> {
    let document = aozora::parse(text).map_err(|source| CheckError::Parse { source })?;
    let mut notation: Vec<(u32, u32)> = document
        .snapshot()
        .nodes()
        .iter()
        .map(|node| (node.span().start, node.span().end))
        .collect();
    notation.sort_unstable();
    let notation = merge_spans(notation);
    let mut findings = Vec::new();
    let mut characters = text.char_indices().peekable();
    let mut previous = None;
    let mut span_index = 0usize;
    while let Some((offset, character)) = characters.next() {
        let source_offset = Span::try_from_usize(offset, offset)?.start;
        while notation
            .get(span_index)
            .is_some_and(|&(_, end)| end <= source_offset)
        {
            span_index = span_index.saturating_add(1);
        }
        let inside_notation = notation
            .get(span_index)
            .is_some_and(|&(start, end)| start <= source_offset && source_offset < end);
        let next = characters.peek().map(|&(_, value)| value);
        if inside_notation {
            previous = Some(character);
            continue;
        }
        match character {
            '～' => findings.push(EXPERIMENTAL_FULLWIDTH_TILDE),
            '(' | ')' if touches_non_ascii(previous, next) => {
                findings.push(EXPERIMENTAL_ASCII_PARENTHESIS);
            }
            ' ' if suspicious_halfwidth_space(previous, next) => {
                findings.push(EXPERIMENTAL_HALFWIDTH_SPACE);
            }
            _ => {}
        }
        previous = Some(character);
    }
    Ok(findings)
}

fn merge_spans(spans: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    let mut merged: Vec<(u32, u32)> = Vec::new();
    for (start, end) in spans {
        if let Some((_, prior_end)) = merged.last_mut()
            && start <= *prior_end
        {
            *prior_end = (*prior_end).max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn touches_non_ascii(previous: Option<char>, next: Option<char>) -> bool {
    previous.is_some_and(|character| !character.is_ascii() && !character.is_whitespace())
        || next.is_some_and(|character| !character.is_ascii() && !character.is_whitespace())
}

const fn suspicious_halfwidth_space(previous: Option<char>, next: Option<char>) -> bool {
    match (previous, next) {
        (None | Some('\n' | '\r'), Some(next)) => !next.is_ascii() && !next.is_whitespace(),
        (Some(previous), None | Some('\n' | '\r')) => {
            !previous.is_ascii() && !previous.is_whitespace()
        }
        (Some(previous), Some(next)) => {
            !previous.is_ascii()
                && !previous.is_whitespace()
                && !next.is_ascii()
                && !next.is_whitespace()
        }
        _ => false,
    }
}

#[derive(Debug, Default)]
struct TraitObjectVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for TraitObjectVisitor {
    fn visit_type_trait_object(&mut self, _node: &'ast syn::TypeTraitObject) {
        self.found = true;
    }
}

fn source_has_trait_object(source: &str) -> Result<bool, syn::Error> {
    let syntax = syn::parse_file(source)?;
    let mut visitor = TraitObjectVisitor::default();
    visitor.visit_file(&syntax);
    Ok(visitor.found)
}

fn lint_rust_policy(root: &Path) -> Result<(), XtaskError> {
    let mut sources = Vec::new();
    collect_rust_sources(root, &mut sources)?;
    sources.sort();
    let mut violations = Vec::new();
    for path in sources {
        let source = fs::read_to_string(&path).map_err(|source| XtaskError::Io {
            path: path.clone(),
            operation: "read repository-owned Rust",
            source,
        })?;
        let has_trait_object =
            source_has_trait_object(&source).map_err(|source| XtaskError::RustSyntax {
                path: path.clone(),
                source,
            })?;
        if has_trait_object {
            violations.push(relative_path(root, &path).into());
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::TraitObjects { paths: violations })
    }
}

fn collect_rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) -> Result<(), XtaskError> {
    let mut entries = read_directory(directory, "read the repository tree")?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| XtaskError::Io {
            path: path.clone(),
            operation: "read a repository entry type",
            source,
        })?;
        if file_type.is_dir() {
            if matches!(
                entry.file_name().to_str(),
                Some(".git" | "target" | "node_modules")
            ) {
                continue;
            }
            collect_rust_sources(&path, sources)?;
        } else if file_type.is_file() && path.extension() == Some(std::ffi::OsStr::new("rs")) {
            sources.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_use_forward_slashes() {
        let root = Path::new("/corpus");
        let path = root.join("author").join("work.txt");
        assert_eq!(relative_path(root, &path), "author/work.txt");
    }

    #[test]
    fn experimental_rules_are_context_sensitive() {
        assert!(
            !experimental_findings("実は［＃「実は」～「思想」に傍点］")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_FULLWIDTH_TILDE)
        );
        assert!(
            experimental_findings("1～3")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_FULLWIDTH_TILDE)
        );
        assert!(
            experimental_findings("青空 文庫")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_HALFWIDTH_SPACE)
        );
        assert!(
            !experimental_findings("Aozora Bunko")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_HALFWIDTH_SPACE)
        );
        assert!(
            experimental_findings("青空(文庫)")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_ASCII_PARENTHESIS)
        );
        assert!(
            !experimental_findings("(ASCII)")
                .expect("experimental scan")
                .contains(&EXPERIMENTAL_ASCII_PARENTHESIS)
        );
    }

    #[test]
    fn merge_spans_collapses_nested_and_adjacent_ranges() {
        assert_eq!(
            merge_spans(vec![(1, 3), (2, 4), (4, 6), (8, 9)]),
            vec![(1, 6), (8, 9)]
        );
    }

    #[test]
    fn codepoint_keys_are_stable_and_visible() {
        assert_eq!(codepoint_key('\t'), "U+0009");
        assert_eq!(codepoint_key('Ⅰ'), "U+2160");
    }

    #[test]
    fn rust_policy_rejects_direct_nested_and_multiline_trait_objects() {
        for source in [
            "type Handler = &dyn Send;",
            "type Handler = Option<Box<dyn Send + Sync>>;",
            "type Handler = Box<\n dyn\n Send\n>;",
        ] {
            assert!(source_has_trait_object(source).expect("valid Rust"));
        }
    }

    #[test]
    fn rust_policy_allows_static_dispatch_enums_and_plain_boxes() {
        for source in [
            "fn run<T: Send>(value: T) { drop(value); }",
            "enum Handler<T> { One(T), Empty }",
            "type Handler<T> = Box<T>;",
        ] {
            assert!(!source_has_trait_object(source).expect("valid Rust"));
        }
    }
}

//! Unpublished development commands for corpus validation.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aozora_proof_core::{FindingSource, run_submission};
use serde::Serialize;

const AUDIT_SCHEMA_VERSION: u32 = 2;
const DEFAULT_SAMPLES: usize = 5;

#[derive(Debug)]
struct Args {
    corpus: PathBuf,
    output: PathBuf,
    samples: usize,
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
        Ok(output) => {
            eprintln!("corpus audit: {}", output.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::from(2)
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("audit")) {
        return Err("usage: cargo xtask audit --corpus ROOT [--out FILE] [--samples N]".to_owned());
    }

    let mut corpus = None;
    let mut output = PathBuf::from("target/corpus-audit.json");
    let mut samples = DEFAULT_SAMPLES;
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--corpus") => {
                corpus = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--corpus requires a path".to_owned())?,
                ));
            }
            Some("--out") => {
                output = PathBuf::from(
                    args.next()
                        .ok_or_else(|| "--out requires a path".to_owned())?,
                );
            }
            Some("--samples") => {
                let value = args
                    .next()
                    .ok_or_else(|| "--samples requires a number".to_owned())?;
                samples = value
                    .to_str()
                    .ok_or_else(|| "--samples must be UTF-8".to_owned())?
                    .parse()
                    .map_err(|_| "--samples must be a non-negative integer".to_owned())?;
            }
            Some(other) => return Err(format!("unknown argument: {other}")),
            None => return Err("arguments must be UTF-8".to_owned()),
        }
    }

    Ok(Args {
        corpus: corpus.ok_or_else(|| "--corpus is required".to_owned())?,
        output,
        samples,
    })
}

fn run(args: Args) -> Result<PathBuf, String> {
    let root = args
        .corpus
        .canonicalize()
        .map_err(|error| format!("{}: {error}", args.corpus.display()))?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()));
    }

    let mut paths = Vec::new();
    collect_text_files(&root, &mut paths)
        .map_err(|error| format!("{}: {error}", root.display()))?;
    paths.sort();

    let mut audit = Audit {
        schema_version: AUDIT_SCHEMA_VERSION,
        corpus: root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or(".")
            .to_owned(),
        files_scanned: 0,
        bytes_scanned: 0,
        files_with_findings: 0,
        internal_findings: 0,
        rules: BTreeMap::new(),
        experimental_rules: BTreeMap::new(),
    };

    for path in paths {
        let bytes = fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let relative = relative_path(&root, &path);
        let report = run_submission(&bytes);
        audit.files_scanned += 1;
        audit.bytes_scanned = audit
            .bytes_scanned
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        if !report.findings.is_empty() {
            audit.files_with_findings += 1;
        }

        let mut file_codes = BTreeSet::new();
        for finding in report.findings {
            if matches!(finding.source, FindingSource::Internal) {
                audit.internal_findings += 1;
            }
            let stats = audit.rules.entry(finding.code.to_owned()).or_default();
            finding
                .severity
                .as_wire_str()
                .clone_into(&mut stats.severity);
            stats.findings += 1;
            record_codepoint(stats, finding.codepoint);
            if file_codes.insert(finding.code) {
                stats.files += 1;
                if stats.samples.len() < args.samples {
                    stats.samples.push(relative.clone());
                }
            }
        }

        let mut experimental_file_codes = BTreeSet::new();
        for code in experimental_findings(&report.decoded) {
            let stats = audit.experimental_rules.entry(code.to_owned()).or_default();
            "experimental".clone_into(&mut stats.severity);
            stats.findings += 1;
            if experimental_file_codes.insert(code) {
                stats.files += 1;
                if stats.samples.len() < args.samples {
                    stats.samples.push(relative.clone());
                }
            }
        }
    }

    if let Some(parent) = args
        .output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(&audit)
        .map_err(|error| format!("could not serialize audit: {error}"))?;
    fs::write(&args.output, json).map_err(|error| format!("{}: {error}", args.output.display()))?;
    Ok(args.output)
}

fn codepoint_key(character: char) -> String {
    format!("U+{:04X}", u32::from(character))
}

fn record_codepoint(stats: &mut RuleStats, codepoint: Option<char>) {
    if let Some(character) = codepoint {
        *stats
            .codepoints
            .entry(codepoint_key(character))
            .or_default() += 1;
    }
}

fn collect_text_files(directory: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
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

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

const EXPERIMENTAL_HALFWIDTH_SPACE: &str = "experimental::halfwidth_space";
const EXPERIMENTAL_ASCII_PARENTHESIS: &str = "experimental::ascii_parenthesis";
const EXPERIMENTAL_FULLWIDTH_TILDE: &str = "experimental::fullwidth_tilde";

fn experimental_findings(text: &str) -> Vec<&'static str> {
    let mut notation = aozora::parse(text).map_or_else(
        |_| Vec::new(),
        |document| {
            document
                .snapshot()
                .nodes()
                .iter()
                .map(|node| (node.span().start, node.span().end))
                .collect()
        },
    );
    notation.sort_unstable();
    let notation = merge_spans(notation);
    let mut findings = Vec::new();
    let mut characters = text.char_indices().peekable();
    let mut previous = None;
    let mut span_index = 0usize;
    while let Some((offset, character)) = characters.next() {
        let source_offset = u32::try_from(offset).unwrap_or(u32::MAX);
        while notation
            .get(span_index)
            .is_some_and(|&(_, end)| end <= source_offset)
        {
            span_index += 1;
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
    findings
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
                .contains(&EXPERIMENTAL_FULLWIDTH_TILDE)
        );
        assert!(experimental_findings("1～3").contains(&EXPERIMENTAL_FULLWIDTH_TILDE));
        assert!(experimental_findings("青空 文庫").contains(&EXPERIMENTAL_HALFWIDTH_SPACE));
        assert!(!experimental_findings("Aozora Bunko").contains(&EXPERIMENTAL_HALFWIDTH_SPACE));
        assert!(experimental_findings("青空(文庫)").contains(&EXPERIMENTAL_ASCII_PARENTHESIS));
        assert!(!experimental_findings("(ASCII)").contains(&EXPERIMENTAL_ASCII_PARENTHESIS));
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
}

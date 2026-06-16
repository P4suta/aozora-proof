//! `aozora-proof` — command-line proofreader for 青空文庫 text.
//!
//! Reads files (or stdin), runs the full [`aozora_proof_core`] pipeline over
//! each, and reports findings in a human, JSON, or short format. Exit codes
//! follow the `aozora` convention (0 clean / 1 findings / 2 usage / 3 internal).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use aozora_proof_core::gaiji_dict::{self, GaijiInfo};
use aozora_proof_core::{
    Finding, FindingSource, Report, SCHEMA_VERSION, Severity, Suggestion, run_all,
};
use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "aozora-proof",
    version,
    about = "青空文庫記法テキストの文字レベル校正ツール（JIS 適合・旧字体/新字体・外字参照）"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// テキストを校正する（記法＋文字レベル）。
    Check(CheckArgs),
    /// 外字（gaiji）を参照・検索する（注記 ⇔ 文字 ⇔ 面区点 ⇔ Unicode）。
    Gaiji(GaijiArgs),
}

#[derive(Args)]
struct CheckArgs {
    /// 入力ファイル（複数可）。`-` で標準入力を読む。
    #[arg(default_value = "-")]
    paths: Vec<PathBuf>,

    /// 出力形式。`auto` は端末なら human、パイプなら json。
    #[arg(long, value_enum, default_value_t = Format::Auto)]
    format: Format,

    /// 1件でも検出されたら異常終了する。
    #[arg(short, long)]
    strict: bool,

    /// この深刻度以上を検出したら異常終了する。
    #[arg(long, value_enum, default_value_t = SeverityArg::Error)]
    fail_on: SeverityArg,

    /// 旧字体→新字体の置換候補をプレビューする（書き込まない）。
    #[arg(long, conflicts_with = "fix")]
    diff: bool,

    /// 旧字体→新字体の置換を適用してファイルに書き戻す（UTF-8 で出力）。
    #[arg(long)]
    fix: bool,
}

#[derive(Args)]
struct GaijiArgs {
    /// 調べたい文字（先頭1文字）。
    character: Option<String>,

    /// 面区点から引く（例: 1-84-25）。
    #[arg(long, value_name = "M-K-T")]
    men_ku_ten: Option<String>,

    /// Unicode 符号位置から引く（例: U+20089）。
    #[arg(long, value_name = "U+XXXX")]
    unicode: Option<String>,

    /// 注記の説明文を部分一致検索する。
    #[arg(long, value_name = "QUERY")]
    search: Option<String>,

    /// 出力形式（auto / human / json）。
    #[arg(long, value_enum, default_value_t = Format::Auto)]
    format: Format,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Auto,
    Human,
    Json,
    Short,
    /// SARIF 2.1.0 — for GitHub code-scanning upload.
    Sarif,
}

#[derive(Clone, Copy, ValueEnum)]
enum SeverityArg {
    Error,
    Warning,
    Note,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => run_check(&args),
        Command::Gaiji(args) => run_gaiji(&args),
    }
}

fn run_check(args: &CheckArgs) -> ExitCode {
    let mut results: Vec<(String, Report)> = Vec::new();
    let mut read_error = false;

    for path in &args.paths {
        match read_input(path) {
            Ok(bytes) => {
                let label = if path.as_os_str() == "-" {
                    "<stdin>".to_owned()
                } else {
                    path.display().to_string()
                };
                results.push((label, run_all(&bytes)));
            }
            Err(err) => {
                eprintln!("aozora-proof: {}: {err}", path.display());
                read_error = true;
            }
        }
    }

    if args.fix || args.diff {
        return run_fix(&results, args, read_error);
    }

    match resolve_format(args.format) {
        Format::Json => print_json(&results),
        Format::Short => print_short(&results),
        Format::Sarif => print_sarif(&results),
        Format::Human | Format::Auto => print_human(&results),
    }

    decide_exit(&results, args, read_error)
}

fn read_input(path: &Path) -> io::Result<Vec<u8>> {
    if path.as_os_str() == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        fs::read(path)
    }
}

fn resolve_format(format: Format) -> Format {
    match format {
        Format::Auto => {
            if io::stdout().is_terminal() {
                Format::Human
            } else {
                Format::Json
            }
        }
        other => other,
    }
}

/// One file's findings, wrapped with its path for machine-readable output.
#[derive(serde::Serialize)]
struct FileReportJson<'a> {
    path: &'a str,
    schema_version: u32,
    data: &'a [Finding],
}

fn print_json(results: &[(String, Report)]) {
    let payload: Vec<FileReportJson<'_>> = results
        .iter()
        .map(|(label, report)| FileReportJson {
            path: label,
            schema_version: SCHEMA_VERSION,
            data: &report.findings,
        })
        .collect();
    let json = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "[]".to_owned());
    println!("{json}");
}

fn print_short(results: &[(String, Report)]) {
    for (label, report) in results {
        for finding in &report.findings {
            let (line, col) = line_col(&report.decoded, finding.span.start);
            println!(
                "{label}:{line}:{col}: {}: {}: {}",
                finding.severity.as_wire_str(),
                finding.code,
                finding.message
            );
        }
    }
}

fn print_human(results: &[(String, Report)]) {
    let mut total = 0usize;
    for (label, report) in results {
        if report.findings.is_empty() {
            continue;
        }
        println!("{label}:");
        for finding in &report.findings {
            let (line, col) = line_col(&report.decoded, finding.span.start);
            println!(
                "  {line}:{col}  {:7}  {}  {}",
                finding.severity.as_wire_str(),
                finding.code,
                finding.message
            );
            total += 1;
        }
        println!();
    }
    if total == 0 {
        println!("✓ 問題は見つかりませんでした。");
    } else {
        println!("{total} 件の指摘が見つかりました。");
    }
}

/// SARIF 2.1.0 report for GitHub code-scanning upload.
fn print_sarif(results: &[(String, Report)]) {
    let mut rules: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    let mut sarif_results: Vec<serde_json::Value> = Vec::new();
    for (label, report) in results {
        for finding in &report.findings {
            rules.entry(finding.code).or_insert_with(|| {
                serde_json::json!({
                    "id": finding.code,
                    "name": finding.kind(),
                    "shortDescription": { "text": finding.code },
                })
            });
            let (start_line, start_col) = line_col(&report.decoded, finding.span.start);
            let (end_line, end_col) = line_col(&report.decoded, finding.span.end);
            sarif_results.push(serde_json::json!({
                "ruleId": finding.code,
                "level": sarif_level(finding.severity),
                "message": { "text": finding.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": label },
                        "region": {
                            "startLine": start_line,
                            "startColumn": start_col,
                            "endLine": end_line,
                            "endColumn": end_col,
                        },
                    },
                }],
            }));
        }
    }
    let doc = serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "aozora-proof",
                    "informationUri": "https://github.com/P4suta/aozora-proof",
                    "rules": rules.into_values().collect::<Vec<_>>(),
                },
            },
            "results": sarif_results,
        }],
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_owned())
    );
}

const fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// 1-based (line, column-in-chars) of a decoded byte offset.
fn line_col(text: &str, byte: u32) -> (usize, usize) {
    let limit = usize::try_from(byte).unwrap_or(usize::MAX);
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in text.char_indices() {
        if idx >= limit {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn decide_exit(results: &[(String, Report)], args: &CheckArgs, read_error: bool) -> ExitCode {
    let all = || results.iter().flat_map(|(_, report)| &report.findings);

    if all().any(|f| matches!(f.source, FindingSource::Internal)) {
        return ExitCode::from(3);
    }
    if read_error {
        return ExitCode::from(2);
    }

    let triggered = if args.strict {
        all().next().is_some()
    } else {
        let threshold = sev_rank_arg(args.fail_on);
        all().any(|f| sev_rank(f.severity) >= threshold)
    };

    if triggered {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

const fn sev_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 3,
        Severity::Warning => 2,
        Severity::Note => 1,
    }
}

const fn sev_rank_arg(severity: SeverityArg) -> u8 {
    match severity {
        SeverityArg::Error => 3,
        SeverityArg::Warning => 2,
        SeverityArg::Note => 1,
    }
}

/// `--diff` / `--fix`: preview or apply the 旧字体→新字体 replacement suggestions.
fn run_fix(results: &[(String, Report)], args: &CheckArgs, read_error: bool) -> ExitCode {
    let mut total = 0usize;
    for (label, report) in results {
        let subs: Vec<&Suggestion> = report
            .findings
            .iter()
            .filter_map(|f| f.suggestion.as_ref())
            .collect();
        if subs.is_empty() {
            continue;
        }
        total += subs.len();
        if args.diff {
            println!("{label}:");
            for s in &subs {
                let (line, col) = line_col(&report.decoded, s.span.start);
                println!("  {line}:{col}  {}", s.label);
            }
        } else {
            let fixed = apply_suggestions(&report.decoded, &subs);
            if label == "<stdin>" {
                print!("{fixed}");
            } else if let Err(err) = fs::write(label, &fixed) {
                eprintln!("aozora-proof: {label}: {err}");
                return ExitCode::from(2);
            }
        }
    }
    if args.fix {
        eprintln!("aozora-proof: applied {total} replacement(s) (written as UTF-8)");
    } else {
        eprintln!("aozora-proof: {total} suggested replacement(s)");
    }
    if read_error {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

/// Apply replacement suggestions to `decoded`, right-to-left so earlier byte
/// offsets stay valid as later spans are replaced.
fn apply_suggestions(decoded: &str, subs: &[&Suggestion]) -> String {
    let mut ordered: Vec<&Suggestion> = subs.to_vec();
    ordered.sort_by_key(|s| core::cmp::Reverse(s.span.start));
    let mut text = decoded.to_owned();
    for s in ordered {
        let start = usize::try_from(s.span.start).unwrap_or(usize::MAX);
        let end = usize::try_from(s.span.end).unwrap_or(usize::MAX);
        if start <= end
            && end <= text.len()
            && text.is_char_boundary(start)
            && text.is_char_boundary(end)
        {
            text.replace_range(start..end, &s.replacement);
        }
    }
    text
}

fn run_gaiji(args: &GaijiArgs) -> ExitCode {
    let json = matches!(resolve_format(args.format), Format::Json);

    if let Some(query) = &args.search {
        let hits = gaiji_dict::search(query);
        if json {
            let matches: Vec<serde_json::Value> = hits
                .iter()
                .map(|&(desc, c)| {
                    serde_json::json!({
                        "description": desc,
                        "char": c.to_string(),
                        "codepoint": format!("U+{:04X}", u32::from(c)),
                    })
                })
                .collect();
            let doc = serde_json::json!({ "query": query, "matches": matches });
            println!(
                "{}",
                serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_owned())
            );
        } else {
            for (desc, c) in &hits {
                println!("{c}\tU+{:04X}\t{desc}", u32::from(*c));
            }
            eprintln!("aozora-proof gaiji: {} match(es)", hits.len());
        }
        return ExitCode::SUCCESS;
    }

    let ch = if let Some(s) = &args.character {
        s.chars().next()
    } else if let Some(mkt) = &args.men_ku_ten {
        parse_men_ku_ten(mkt).and_then(|(m, k, t)| gaiji_dict::from_men_ku_ten(m, k, t))
    } else if let Some(u) = &args.unicode {
        parse_unicode(u)
    } else {
        eprintln!(
            "aozora-proof gaiji: 文字 / --men-ku-ten / --unicode / --search のいずれかを指定してください"
        );
        return ExitCode::from(2);
    };

    ch.map_or_else(
        || {
            eprintln!("aozora-proof gaiji: 該当する文字が見つかりません");
            ExitCode::from(1)
        },
        |ch| {
            print_gaiji_info(&gaiji_dict::lookup(ch), json);
            ExitCode::SUCCESS
        },
    )
}

/// Parse a `men-ku-ten` string like `1-84-25`.
fn parse_men_ku_ten(s: &str) -> Option<(u8, u8, u8)> {
    let mut parts = s.split('-');
    let men = parts.next()?.parse().ok()?;
    let ku = parts.next()?.parse().ok()?;
    let ten = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((men, ku, ten))
}

/// Parse `U+XXXX` (or a bare hex codepoint) into a `char`.
fn parse_unicode(s: &str) -> Option<char> {
    let hex = s
        .strip_prefix("U+")
        .or_else(|| s.strip_prefix("u+"))
        .unwrap_or(s);
    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
}

fn print_gaiji_info(info: &GaijiInfo, json: bool) {
    if json {
        let men_ku_ten = info.men_ku_ten.map(|m| {
            serde_json::json!({ "men": m.men, "ku": m.ku, "ten": m.ten, "level": m.level.label() })
        });
        let doc = serde_json::json!({
            "char": info.character.to_string(),
            "codepoint": format!("U+{:04X}", info.codepoint),
            "menKuTen": men_ku_ten,
            "descriptions": info.descriptions,
            "chuki": info.chuki,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_owned())
        );
    } else {
        println!("文字: {}  (U+{:04X})", info.character, info.codepoint);
        match info.men_ku_ten {
            Some(m) => println!(
                "面区点: {}-{}-{}  ({})",
                m.men,
                m.ku,
                m.ten,
                m.level.label()
            ),
            None => println!("面区点: —（JIS X 0213 外）"),
        }
        if !info.descriptions.is_empty() {
            println!("注記説明: {}", info.descriptions.join(" / "));
        }
        if let Some(chuki) = &info.chuki {
            println!("外字注記: {chuki}");
        }
    }
}

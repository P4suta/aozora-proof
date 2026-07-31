//! `aozora-proof` command-line application.

#![forbid(unsafe_code)]
#![allow(
    clippy::redundant_pub_crate,
    reason = "the binary is split into crate-visible implementation modules"
)]

pub(crate) mod cli;
pub(crate) mod config;
pub(crate) mod discovery;
pub(crate) mod document;
pub(crate) mod fix_command;
pub(crate) mod output;
pub(crate) mod review;

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::mpsc::channel;
use std::time::Duration;

use aozora_proof_core::gaiji_dict::{self, GaijiInfo};
use aozora_proof_core::{
    DetectionClass, FindingSource, RuleDoc, Severity, all_rules, explain, official_items,
};
use clap::{CommandFactory, Parser};
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

use crate::cli::{
    CheckArgs, Cli, ColorChoice, Command, ConfigCommand, FixArgs, Format, GaijiCommand,
    LanguageArg, ReviewArgs, SeverityArg,
};
use crate::config::{FlagValues, Resolved};
use crate::document::Document;
use crate::output::Painter;

const LONG_VERSION: &str = env!("AOZORA_PROOF_LONG_VERSION");

fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(outcome) => match write_stdout(&outcome.stdout) {
            Ok(()) => ExitCode::from(outcome.code),
            Err(source) if source.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(source) => {
                eprintln!("aozora-proof: {source}");
                ExitCode::from(2)
            }
        },
        Err(error) => {
            eprintln!("aozora-proof: {error}");
            ExitCode::from(error.code())
        }
    }
}

#[derive(Debug)]
struct Outcome {
    stdout: Vec<u8>,
    code: u8,
}

impl Outcome {
    fn success(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            stdout: stdout.into(),
            code: 0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("{message}")]
    Usage { message: String },
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    Discovery(#[from] discovery::DiscoveryError),
    #[error(transparent)]
    Document(#[from] document::DocumentError),
    #[error(transparent)]
    Fix(#[from] fix_command::FixCommandError),
    #[error(transparent)]
    Review(#[from] review::ReviewError),
    #[error(transparent)]
    Render(#[from] output::RenderError),
}

impl AppError {
    const fn code(&self) -> u8 {
        match self {
            Self::Render(_) => 3,
            Self::Document(source) if source.is_internal() => 3,
            Self::Fix(source) if source.is_internal() => 3,
            Self::Review(source) if source.is_internal() => 3,
            Self::Usage { .. }
            | Self::Config(_)
            | Self::Discovery(_)
            | Self::Document(_)
            | Self::Fix(_)
            | Self::Review(_) => 2,
        }
    }
}

fn dispatch(cli: Cli) -> Result<Outcome, AppError> {
    let global_color = cli.color;
    let global_language = cli.lang;
    match cli.command {
        Command::Check(args) => run_check(&args, global_color, global_language),
        Command::Fix(args) => run_fix(&args, global_color, global_language),
        Command::Review(args) => run_review(&args, global_color, global_language),
        Command::Explain { code } => {
            let settings = reference_settings(global_color, global_language)?;
            run_explain(&code, &settings)
        }
        Command::Gaiji { command } => {
            let settings = reference_settings(global_color, global_language)?;
            run_gaiji(command, &settings)
        }
        Command::Rules { format } => {
            let mut settings = reference_settings(global_color, global_language)?;
            if let Some(value) = format {
                settings.format = value;
            }
            Ok(run_rules(&settings))
        }
        Command::Init(args) => {
            let path = config::init(args.user)?;
            Ok(Outcome::success(
                format!("created {}\n", path.display()).into_bytes(),
            ))
        }
        Command::Config { command } => run_config(command, global_color, global_language),
        Command::Completions { shell } => {
            let mut command = Cli::command();
            let name = command.get_name().to_owned();
            let mut output = Vec::new();
            clap_complete::generate(shell, &mut command, name, &mut output);
            Ok(Outcome::success(output))
        }
        Command::Man => {
            let mut output = Vec::new();
            clap_mangen::Man::new(Cli::command())
                .render(&mut output)
                .map_err(usage)?;
            Ok(Outcome::success(output))
        }
    }
}

fn run_check(
    args: &CheckArgs,
    color: Option<ColorChoice>,
    language: Option<LanguageArg>,
) -> Result<Outcome, AppError> {
    let flags = FlagValues {
        orthography: args.document.orthography,
        fail_on: args.fail_on,
        format: args.format,
        color,
        language,
    };
    let mut settings = config::resolve(&args.paths, args.document.config.as_deref(), flags)?;
    let has_stdin = paths_have_stdin(&args.paths);
    config::require_orthography(&mut settings, !has_stdin && !args.document.no_input)?;
    if args.watch {
        return run_watch(args, flags, &settings);
    }
    check_once(&args.paths, &settings)
}

fn check_once(paths: &[PathBuf], settings: &Resolved) -> Result<Outcome, AppError> {
    let inputs = discovery::discover(paths, settings)?;
    let documents = document::load(&inputs, settings)?;
    let stdout = output::render(
        &documents,
        settings.format,
        settings.language,
        Painter::resolve(settings.color),
    )?;
    let code = check_exit(&documents, settings.fail_on);
    Ok(Outcome { stdout, code })
}

fn run_fix(
    args: &FixArgs,
    color: Option<ColorChoice>,
    language: Option<LanguageArg>,
) -> Result<Outcome, AppError> {
    let flags = FlagValues {
        orthography: args.document.orthography,
        color,
        language,
        ..FlagValues::default()
    };
    let mut settings = config::resolve(&args.paths, args.document.config.as_deref(), flags)?;
    config::require_orthography(
        &mut settings,
        !paths_have_stdin(&args.paths) && !args.document.no_input,
    )?;
    let inputs = discovery::discover(&args.paths, &settings)?;
    let documents = document::load(&inputs, &settings)?;
    let output = fix_command::run(&documents, &settings, args.dry_run)?;
    if output.changed_files > 0 && !args.dry_run && !inputs.iter().any(discovery::Input::is_stdin) {
        eprintln!("aozora-proof: fixed {} file(s)", output.changed_files);
    }
    Ok(Outcome::success(output.stdout))
}

fn run_review(
    args: &ReviewArgs,
    color: Option<ColorChoice>,
    language: Option<LanguageArg>,
) -> Result<Outcome, AppError> {
    if args.paths.is_empty() || paths_have_stdin(&args.paths) {
        return Err(usage("review requires one or more files or directories"));
    }
    let flags = FlagValues {
        orthography: args.document.orthography,
        color,
        language,
        ..FlagValues::default()
    };
    let mut settings = config::resolve(&args.paths, args.document.config.as_deref(), flags)?;
    config::require_orthography(&mut settings, !args.document.no_input)?;
    let inputs = discovery::discover(&args.paths, &settings)?;
    if inputs.iter().any(discovery::Input::is_stdin) {
        return Err(usage("review does not accept standard input"));
    }
    let documents = document::load(&inputs, &settings)?;
    let changed = review::run(&documents)?;
    if changed > 0 {
        eprintln!("aozora-proof: wrote {changed} reviewed file(s)");
    }
    Ok(Outcome::success(Vec::new()))
}

fn run_watch(args: &CheckArgs, flags: FlagValues, initial: &Resolved) -> Result<Outcome, AppError> {
    if !io::stdout().is_terminal() || !io::stderr().is_terminal() {
        return Err(usage("--watch requires terminal stdout and stderr"));
    }
    if output::resolve_format(initial.format) != Format::Human {
        return Err(usage("--watch requires --format human or auto"));
    }
    if paths_have_stdin(&args.paths) {
        return Err(usage("--watch does not accept standard input"));
    }

    let (sender, receiver) = channel();
    let mut debouncer = new_debouncer(Duration::from_millis(250), sender).map_err(usage)?;
    let mut watched = BTreeSet::new();
    for path in &args.paths {
        let metadata = fs::metadata(path).map_err(usage)?;
        let target = if metadata.is_file() {
            path.parent().unwrap_or_else(|| Path::new("."))
        } else {
            path.as_path()
        };
        let mode = if metadata.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if watched.insert(target.to_path_buf()) {
            debouncer.watcher().watch(target, mode).map_err(usage)?;
        }
    }
    for config_path in [
        initial.project_config.as_deref(),
        Some(initial.user_config.as_path()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(parent) = config_path.parent()
            && parent.is_dir()
            && watched.insert(parent.to_path_buf())
        {
            debouncer
                .watcher()
                .watch(parent, RecursiveMode::NonRecursive)
                .map_err(usage)?;
        }
    }

    eprintln!("aozora-proof: watching for document and configuration changes");
    let first = check_once(&args.paths, initial)?;
    write_watch_frame(&first.stdout).map_err(usage)?;
    loop {
        receiver
            .recv()
            .map_err(|source| usage(source.to_string()))?
            .map_err(usage)?;
        let mut settings = config::resolve(&args.paths, args.document.config.as_deref(), flags)?;
        config::require_orthography(&mut settings, false)?;
        let outcome = check_once(&args.paths, &settings)?;
        match write_watch_frame(&outcome.stdout) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::BrokenPipe => {
                return Ok(Outcome::success(Vec::new()));
            }
            Err(source) => return Err(usage(source)),
        }
    }
}

fn write_watch_frame(bytes: &[u8]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(b"\x1b[2J\x1b[H")?;
    lock.write_all(bytes)?;
    lock.flush()
}

fn reference_settings(
    color: Option<ColorChoice>,
    language: Option<LanguageArg>,
) -> Result<Resolved, AppError> {
    config::resolve(
        &[],
        None,
        FlagValues {
            color,
            language,
            ..FlagValues::default()
        },
    )
    .map_err(AppError::from)
}

fn run_explain(code: &str, settings: &Resolved) -> Result<Outcome, AppError> {
    let Some(rule) = explain(code) else {
        return Err(usage(format!("unknown rule code {code}")));
    };
    Ok(Outcome::success(
        format_rule(&rule, settings.language).into_bytes(),
    ))
}

fn format_rule(rule: &RuleDoc, language: LanguageArg) -> String {
    let japanese = language == LanguageArg::Ja;
    let title = if japanese { rule.title_ja } else { rule.title };
    let rationale = if japanese {
        rule.rationale_ja
    } else {
        rule.rationale
    };
    format!(
        "{title}\n{}\n\n{rationale}\n\nbad: {}\ngood: {}\n\
         detection: {}\nfix: {}\nauthority: {}\n",
        rule.code,
        rule.example_bad,
        rule.example_good,
        rule.detection.as_wire_str(),
        rule.fix.map_or("none", |value| value.as_wire_str()),
        rule.authority_url,
    )
}

fn run_rules(settings: &Resolved) -> Outcome {
    if output::resolve_format(settings.format) == Format::Json {
        let rules: Vec<_> = all_rules()
            .iter()
            .map(|rule| {
                serde_json::json!({
                    "code": rule.code,
                    "category": rule.category.as_wire_str(),
                    "severity": rule.default_severity.as_wire_str(),
                    "title": rule.title,
                    "detection": rule.detection.as_wire_str(),
                    "fix": rule.fix.map(aozora_proof_core::FixApplicability::as_wire_str),
                    "authorityUrl": rule.authority_url,
                    "exampleBad": rule.example_bad,
                    "exampleGood": rule.example_good,
                })
            })
            .collect();
        let coverage: Vec<_> = official_items()
            .iter()
            .map(|item| {
                serde_json::json!({
                    "id": item.id,
                    "title": item.title,
                    "detection": item.detection.as_wire_str(),
                    "rules": item.rules,
                    "authorityUrl": item.authority_url,
                })
            })
            .collect();
        let mut text =
            serde_json::json!({ "rules": rules, "officialCoverage": coverage }).to_string();
        text.push('\n');
        return Outcome::success(text.into_bytes());
    }

    let japanese = settings.language == LanguageArg::Ja;
    let mut text = String::new();
    for class in [
        DetectionClass::Automatic,
        DetectionClass::Review,
        DetectionClass::Manual,
    ] {
        text.push_str(class.as_wire_str());
        text.push_str(":\n");
        for rule in all_rules().iter().filter(|rule| rule.detection == class) {
            let title = if japanese { rule.title_ja } else { rule.title };
            text.push_str("  ");
            text.push_str(rule.code);
            text.push_str("  ");
            text.push_str(title);
            text.push('\n');
        }
    }
    Outcome::success(text.into_bytes())
}

fn run_gaiji(command: GaijiCommand, settings: &Resolved) -> Result<Outcome, AppError> {
    match command {
        GaijiCommand::Lookup { query } => {
            let character = parse_gaiji_query(&query)
                .ok_or_else(|| usage(format!("no character found for {query:?}")))?;
            Ok(Outcome::success(
                format_gaiji(&gaiji_dict::lookup(character), settings.language).into_bytes(),
            ))
        }
        GaijiCommand::Search { text } => {
            let mut output = String::new();
            for (description, character) in gaiji_dict::search(&text) {
                output.push(character);
                output.push_str("\tU+");
                let codepoint = format!("{:04X}", u32::from(character));
                output.push_str(&codepoint);
                output.push('\t');
                output.push_str(description);
                output.push('\n');
            }
            Ok(Outcome::success(output.into_bytes()))
        }
    }
}

fn parse_gaiji_query(query: &str) -> Option<char> {
    if let Some(hex) = query
        .strip_prefix("U+")
        .or_else(|| query.strip_prefix("u+"))
    {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    let parts: Vec<_> = query.split('-').collect();
    if let [men, ku, ten] = parts.as_slice() {
        let men = men.parse().ok()?;
        let ku = ku.parse().ok()?;
        let ten = ten.parse().ok()?;
        return gaiji_dict::from_men_ku_ten(men, ku, ten);
    }
    let mut characters = query.chars();
    let character = characters.next()?;
    characters.next().is_none().then_some(character)
}

fn format_gaiji(info: &GaijiInfo, language: LanguageArg) -> String {
    let japanese = language == LanguageArg::Ja;
    let mut output = String::new();
    let scalar_label = if japanese { "文字" } else { "character" };
    output.push_str(scalar_label);
    output.push_str(": ");
    output.push(info.character);
    output.push_str(" (U+");
    let codepoint = format!("{:04X}", info.codepoint);
    output.push_str(&codepoint);
    output.push_str(")\n");
    if let Some(position) = info.men_ku_ten {
        output.push_str("men-ku-ten: ");
        output.push_str(&position.men.to_string());
        output.push('-');
        output.push_str(&position.ku.to_string());
        output.push('-');
        output.push_str(&position.ten.to_string());
        output.push_str(" (");
        output.push_str(position.level.label());
        output.push_str(")\n");
    }
    for description in &info.descriptions {
        output.push_str("description: ");
        output.push_str(description);
        output.push('\n');
    }
    if let Some(annotation) = &info.chuki {
        output.push_str("annotation: ");
        output.push_str(annotation);
        output.push('\n');
    }
    output
}

fn run_config(
    command: ConfigCommand,
    color: Option<ColorChoice>,
    language: Option<LanguageArg>,
) -> Result<Outcome, AppError> {
    match command {
        ConfigCommand::Schema => {
            let mut schema = config::schema_json();
            schema.push('\n');
            Ok(Outcome::success(schema.into_bytes()))
        }
        ConfigCommand::Show { path } => {
            let paths = path.into_iter().collect::<Vec<_>>();
            let settings = config::resolve(
                &paths,
                None,
                FlagValues {
                    color,
                    language,
                    ..FlagValues::default()
                },
            )?;
            Ok(Outcome::success(settings.show().into_bytes()))
        }
    }
}

fn check_exit(documents: &[Document], threshold: SeverityArg) -> u8 {
    if documents.iter().any(|document| {
        document
            .report
            .findings
            .iter()
            .any(|finding| finding.source == FindingSource::Internal)
    }) {
        return 3;
    }
    u8::from(documents.iter().any(|document| {
        document
            .report
            .findings
            .iter()
            .any(|finding| severity_rank(finding.severity) >= threshold.rank())
    }))
}

const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 3,
        Severity::Warning => 2,
        Severity::Note => 1,
    }
}

fn paths_have_stdin(paths: &[PathBuf]) -> bool {
    paths.is_empty() || paths.iter().any(|path| path.as_os_str() == "-")
}

fn write_stdout(bytes: &[u8]) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(bytes)?;
    lock.flush()
}

fn usage(source: impl fmt::Display) -> AppError {
    AppError::Usage {
        message: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use aozora_proof_core::CheckError;

    use super::*;

    #[test]
    fn cli_error_classes_keep_internal_failures_distinct() {
        let external = AppError::Document(document::DocumentError::Check {
            label: "<stdin>".to_owned(),
            source: CheckError::SourceTooLarge { len: usize::MAX },
        });
        let internal = AppError::Document(document::DocumentError::Check {
            label: "<stdin>".to_owned(),
            source: CheckError::UnknownRule {
                code: "aozora::proof::unknown",
            },
        });

        assert_eq!(external.code(), 2);
        assert_eq!(internal.code(), 3);
        assert_eq!(
            AppError::Render(output::RenderError::UnresolvedFormat).code(),
            3
        );
    }
}

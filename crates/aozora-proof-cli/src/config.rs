use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use aozora_proof_core::{Orthography, Report, Severity, all_rules};
use serde::Deserialize;

use crate::cli::{ColorChoice, Format, LanguageArg, OrthographyArg, SeverityArg};

const PROJECT_FILE: &str = ".aozora-proof.toml";

#[derive(Debug)]
pub(crate) struct ConfigError {
    message: String,
}

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ConfigError {}

#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub(crate) orthography: Option<Orthography>,
    pub(crate) fail_on: SeverityArg,
    pub(crate) format: Format,
    pub(crate) color: ColorChoice,
    pub(crate) language: LanguageArg,
    pub(crate) include: Vec<String>,
    pub(crate) exclude: Vec<String>,
    pub(crate) respect_ignore: bool,
    pub(crate) autofix: bool,
    pub(crate) project_config: Option<PathBuf>,
    pub(crate) user_config: PathBuf,
    pub(crate) sources: Sources,
    rule_levels: BTreeMap<String, RuleLevel>,
    overrides: Vec<PathOverride>,
}

impl Resolved {
    pub(crate) fn orthography_for(&self, path: &str) -> Option<Orthography> {
        self.overrides
            .iter()
            .filter(|item| wildcard_match(&item.path, path))
            .filter_map(|item| item.orthography.as_deref())
            .filter_map(|value| parse_orthography(value).ok())
            .next_back()
            .or(self.orthography)
    }

    pub(crate) fn autofix_for(&self, path: &str) -> bool {
        self.overrides
            .iter()
            .filter(|item| wildcard_match(&item.path, path))
            .filter_map(|item| item.autofix)
            .next_back()
            .unwrap_or(self.autofix)
    }

    pub(crate) fn apply_rule_levels(&self, path: &str, report: &mut Report) {
        let mut levels = self.rule_levels.clone();
        for item in &self.overrides {
            if wildcard_match(&item.path, path) {
                levels.extend(item.rules.clone());
            }
        }
        report
            .findings
            .retain_mut(|finding| match levels.get(finding.code).copied() {
                Some(RuleLevel::Off) => false,
                Some(level) => {
                    finding.severity = level.severity().unwrap_or(finding.severity);
                    true
                }
                None => true,
            });
    }

    pub(crate) fn show(&self) -> String {
        let orthography = self.orthography.map_or_else(
            || "<required>".to_owned(),
            |value| value.as_str().to_owned(),
        );
        format!(
            "orthography = {orthography}    # {}\n\
             fail-on = {}    # {}\n\
             format = {}    # {}\n\
             color = {}    # {}\n\
             lang = {}    # {}\n\
             respect-ignore = {}    # {}\n\
             autofix = {}    # {}\n\
             project-config = {}\n\
             user-config = {}\n",
            self.sources.orthography,
            severity_name(self.fail_on),
            self.sources.fail_on,
            format_name(self.format),
            self.sources.format,
            color_name(self.color),
            self.sources.color,
            language_name(self.language),
            self.sources.language,
            self.respect_ignore,
            self.sources.respect_ignore,
            self.autofix,
            self.sources.autofix,
            self.project_config
                .as_deref()
                .map_or_else(|| "<none>".to_owned(), |path| path.display().to_string()),
            self.user_config.display(),
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Sources {
    orthography: String,
    fail_on: String,
    format: String,
    color: String,
    language: String,
    respect_ignore: String,
    autofix: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
struct ConfigFile {
    orthography: Option<String>,
    fail_on: Option<String>,
    format: Option<String>,
    color: Option<String>,
    lang: Option<String>,
    include: Vec<String>,
    exclude: Vec<String>,
    respect_ignore: Option<bool>,
    autofix: Option<bool>,
    rules: BTreeMap<String, RuleLevel>,
    overrides: Vec<PathOverride>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PathOverride {
    path: String,
    orthography: Option<String>,
    autofix: Option<bool>,
    rules: BTreeMap<String, RuleLevel>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RuleLevel {
    Error,
    Warning,
    Note,
    Off,
}

impl RuleLevel {
    const fn severity(self) -> Option<Severity> {
        match self {
            Self::Error => Some(Severity::Error),
            Self::Warning => Some(Severity::Warning),
            Self::Note => Some(Severity::Note),
            Self::Off => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlagValues {
    pub(crate) orthography: Option<OrthographyArg>,
    pub(crate) fail_on: Option<SeverityArg>,
    pub(crate) format: Option<Format>,
    pub(crate) color: Option<ColorChoice>,
    pub(crate) language: Option<LanguageArg>,
}

#[derive(Debug, Clone, Copy)]
struct Values {
    orthography: Option<Orthography>,
    fail_on: SeverityArg,
    format: Format,
    color: ColorChoice,
    language: LanguageArg,
}

fn load_configuration(
    paths: &[PathBuf],
    explicit_config: Option<&Path>,
) -> Result<(PathBuf, Option<PathBuf>, ConfigFile, Sources), ConfigError> {
    let user_config = user_config_path()?;
    let project_config = explicit_config
        .map(Path::to_path_buf)
        .or_else(|| find_project_config(paths));
    let mut merged = ConfigFile::default();
    let mut sources = Sources {
        orthography: "unset".to_owned(),
        fail_on: "default".to_owned(),
        format: "default".to_owned(),
        color: "default".to_owned(),
        language: "LANG/default".to_owned(),
        respect_ignore: "default".to_owned(),
        autofix: "default".to_owned(),
    };

    if user_config.is_file() {
        let config = load(&user_config)?;
        merge(
            &mut merged,
            config,
            &mut sources,
            &user_config.display().to_string(),
        );
    }
    if let Some(path) = &project_config {
        if path.is_file() {
            let config = load(path)?;
            merge(
                &mut merged,
                config,
                &mut sources,
                &path.display().to_string(),
            );
        } else if explicit_config.is_some() {
            return Err(ConfigError::new(format!(
                "configuration file does not exist: {}",
                path.display()
            )));
        }
    }
    Ok((user_config, project_config, merged, sources))
}

fn values_from_config(config: &ConfigFile) -> Result<Values, ConfigError> {
    Ok(Values {
        orthography: config
            .orthography
            .as_deref()
            .map(parse_orthography)
            .transpose()?,
        fail_on: config
            .fail_on
            .as_deref()
            .map(parse_severity)
            .transpose()?
            .unwrap_or(SeverityArg::Error),
        format: config
            .format
            .as_deref()
            .map(parse_format)
            .transpose()?
            .unwrap_or(Format::Auto),
        color: config
            .color
            .as_deref()
            .map(parse_color)
            .transpose()?
            .unwrap_or(ColorChoice::Auto),
        language: config
            .lang
            .as_deref()
            .map(parse_language)
            .transpose()?
            .unwrap_or_else(language_from_lang),
    })
}

fn apply_flags(values: &mut Values, sources: &mut Sources, flags: FlagValues) {
    if let Some(value) = flags.orthography {
        values.orthography = Some(value.into());
        "flag --orthography".clone_into(&mut sources.orthography);
    }
    if let Some(value) = flags.fail_on {
        values.fail_on = value;
        "flag --fail-on".clone_into(&mut sources.fail_on);
    }
    if let Some(value) = flags.format {
        values.format = value;
        "flag --format".clone_into(&mut sources.format);
    }
    if let Some(value) = flags.color {
        values.color = value;
        "flag --color".clone_into(&mut sources.color);
    }
    if let Some(value) = flags.language {
        values.language = value;
        "flag --lang".clone_into(&mut sources.language);
    }
}

pub(crate) fn resolve(
    paths: &[PathBuf],
    explicit_config: Option<&Path>,
    flags: FlagValues,
) -> Result<Resolved, ConfigError> {
    let (user_config, project_config, merged, mut sources) =
        load_configuration(paths, explicit_config)?;
    validate_rule_codes(&merged)?;
    let mut values = values_from_config(&merged)?;
    apply_env(&mut values, &mut sources)?;
    apply_flags(&mut values, &mut sources, flags);

    Ok(Resolved {
        orthography: values.orthography,
        fail_on: values.fail_on,
        format: values.format,
        color: values.color,
        language: values.language,
        include: merged.include,
        exclude: merged.exclude,
        respect_ignore: merged.respect_ignore.unwrap_or(true),
        autofix: merged.autofix.unwrap_or(true),
        project_config,
        user_config,
        sources,
        rule_levels: merged.rules,
        overrides: merged.overrides,
    })
}

pub(crate) fn require_orthography(
    resolved: &mut Resolved,
    allow_prompt: bool,
) -> Result<(), ConfigError> {
    if resolved.orthography.is_some() {
        return Ok(());
    }
    if !allow_prompt
        || env::var_os("CI").is_some()
        || !io::stdin().is_terminal()
        || !io::stderr().is_terminal()
    {
        return Err(ConfigError::new(
            "orthography is required; pass --orthography modern|traditional|mixed",
        ));
    }
    eprint!("Orthography [modern/traditional/mixed]: ");
    io::stderr()
        .flush()
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    resolved.orthography = Some(parse_orthography(answer.trim())?);
    "interactive prompt".clone_into(&mut resolved.sources.orthography);
    Ok(())
}

pub(crate) fn init(user: bool) -> Result<PathBuf, ConfigError> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        return Err(ConfigError::new("init requires an interactive terminal"));
    }
    eprint!("Orthography [modern/traditional/mixed]: ");
    io::stderr()
        .flush()
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| ConfigError::new(error.to_string()))?;
    let orthography = parse_orthography(answer.trim())?;
    let path = if user {
        user_config_path()?
    } else {
        env::current_dir()
            .map_err(|error| ConfigError::new(error.to_string()))?
            .join(PROJECT_FILE)
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ConfigError::new(error.to_string()))?;
    }
    let body = format!(
        "orthography = \"{}\"\nfail-on = \"error\"\nformat = \"auto\"\nlang = \"en\"\n",
        orthography.as_str()
    );
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(&path)
        .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
    file.write_all(body.as_bytes())
        .map_err(|error| ConfigError::new(error.to_string()))?;
    Ok(path)
}

pub(crate) fn schema_json() -> String {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "aozora-proof configuration",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "orthography": { "enum": ["modern", "traditional", "mixed"] },
            "fail-on": { "enum": ["error", "warning", "note"] },
            "format": { "enum": ["auto", "human", "json", "short", "sarif"] },
            "color": { "enum": ["auto", "always", "never"] },
            "lang": { "enum": ["en", "ja"] },
            "include": { "type": "array", "items": { "type": "string" } },
            "exclude": { "type": "array", "items": { "type": "string" } },
            "respect-ignore": { "type": "boolean" },
            "autofix": { "type": "boolean" },
            "rules": {
                "type": "object",
                "additionalProperties": { "enum": ["error", "warning", "note", "off"] }
            },
            "overrides": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["path"],
                    "properties": {
                        "path": { "type": "string" },
                        "orthography": { "enum": ["modern", "traditional", "mixed"] },
                        "autofix": { "type": "boolean" },
                        "rules": {
                            "type": "object",
                            "additionalProperties": {
                                "enum": ["error", "warning", "note", "off"]
                            }
                        }
                    }
                }
            }
        }
    })
    .to_string()
}

pub(crate) fn user_config_path() -> Result<PathBuf, ConfigError> {
    if let Some(base) = env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(base).join("aozora-proof").join("config.toml"));
    }
    #[cfg(target_os = "windows")]
    if let Some(base) = env::var_os("APPDATA") {
        return Ok(PathBuf::from(base).join("aozora-proof").join("config.toml"));
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("aozora-proof")
            .join("config.toml"));
    }
    env::var_os("HOME").map_or_else(
        || {
            Err(ConfigError::new(
                "cannot determine the user configuration directory",
            ))
        },
        |home| {
            Ok(PathBuf::from(home)
                .join(".config")
                .join("aozora-proof")
                .join("config.toml"))
        },
    )
}

fn find_project_config(paths: &[PathBuf]) -> Option<PathBuf> {
    let current = env::current_dir().ok()?;
    let first = paths
        .iter()
        .find(|path| path.as_os_str() != "-")
        .map_or_else(
            || current.clone(),
            |path| {
                if path.is_absolute() {
                    path.clone()
                } else {
                    current.join(path)
                }
            },
        );
    let start = if first.is_file() {
        first.parent().map_or(current.as_path(), |path| path)
    } else {
        first.as_path()
    };
    start
        .ancestors()
        .map(|ancestor| ancestor.join(PROJECT_FILE))
        .find(|candidate| candidate.is_file())
}

fn load(path: &Path) -> Result<ConfigFile, ConfigError> {
    let content = fs::read_to_string(path)
        .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
    let value: toml::Value = toml::from_str(&content)
        .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))?;
    validate_config_keys(&value)?;
    value
        .try_into()
        .map_err(|error| ConfigError::new(format!("{}: {error}", path.display())))
}

fn validate_config_keys(value: &toml::Value) -> Result<(), ConfigError> {
    const ROOT_KEYS: &[&str] = &[
        "orthography",
        "fail-on",
        "format",
        "color",
        "lang",
        "include",
        "exclude",
        "respect-ignore",
        "autofix",
        "rules",
        "overrides",
    ];
    const OVERRIDE_KEYS: &[&str] = &["path", "orthography", "autofix", "rules"];

    let Some(table) = value.as_table() else {
        return Err(ConfigError::new("configuration root must be a table"));
    };
    validate_table_keys(table, ROOT_KEYS, "configuration")?;
    if let Some(overrides) = table.get("overrides").and_then(toml::Value::as_array) {
        for override_value in overrides {
            if let Some(override_table) = override_value.as_table() {
                validate_table_keys(override_table, OVERRIDE_KEYS, "override")?;
            }
        }
    }
    Ok(())
}

fn validate_table_keys(
    table: &toml::Table,
    known: &[&str],
    context: &str,
) -> Result<(), ConfigError> {
    for key in table.keys() {
        if known.contains(&key.as_str()) {
            continue;
        }
        let suggestion = known
            .iter()
            .min_by_key(|candidate| levenshtein(key, candidate))
            .copied()
            .unwrap_or_default();
        return Err(ConfigError::new(format!(
            "unknown {context} key {key:?}; did you mean {suggestion:?}?"
        )));
    }
    Ok(())
}

fn merge(target: &mut ConfigFile, source: ConfigFile, sources: &mut Sources, label: &str) {
    if source.orthography.is_some() {
        target.orthography = source.orthography;
        label.clone_into(&mut sources.orthography);
    }
    if source.fail_on.is_some() {
        target.fail_on = source.fail_on;
        label.clone_into(&mut sources.fail_on);
    }
    if source.format.is_some() {
        target.format = source.format;
        label.clone_into(&mut sources.format);
    }
    if source.color.is_some() {
        target.color = source.color;
        label.clone_into(&mut sources.color);
    }
    if source.lang.is_some() {
        target.lang = source.lang;
        label.clone_into(&mut sources.language);
    }
    if source.respect_ignore.is_some() {
        target.respect_ignore = source.respect_ignore;
        label.clone_into(&mut sources.respect_ignore);
    }
    if source.autofix.is_some() {
        target.autofix = source.autofix;
        label.clone_into(&mut sources.autofix);
    }
    if !source.include.is_empty() {
        target.include = source.include;
    }
    if !source.exclude.is_empty() {
        target.exclude = source.exclude;
    }
    target.rules.extend(source.rules);
    target.overrides.extend(source.overrides);
}

fn apply_env(values: &mut Values, sources: &mut Sources) -> Result<(), ConfigError> {
    if let Some(value) = env_value("AOZORA_PROOF_ORTHOGRAPHY") {
        values.orthography = Some(parse_orthography(&value)?);
        "AOZORA_PROOF_ORTHOGRAPHY".clone_into(&mut sources.orthography);
    }
    if let Some(value) = env_value("AOZORA_PROOF_FAIL_ON") {
        values.fail_on = parse_severity(&value)?;
        "AOZORA_PROOF_FAIL_ON".clone_into(&mut sources.fail_on);
    }
    if let Some(value) = env_value("AOZORA_PROOF_FORMAT") {
        values.format = parse_format(&value)?;
        "AOZORA_PROOF_FORMAT".clone_into(&mut sources.format);
    }
    if let Some(value) = env_value("AOZORA_PROOF_COLOR") {
        values.color = parse_color(&value)?;
        "AOZORA_PROOF_COLOR".clone_into(&mut sources.color);
    }
    if let Some(value) = env_value("AOZORA_PROOF_LANG") {
        values.language = parse_language(&value)?;
        "AOZORA_PROOF_LANG".clone_into(&mut sources.language);
    }
    Ok(())
}

fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn validate_rule_codes(config: &ConfigFile) -> Result<(), ConfigError> {
    for code in config
        .rules
        .keys()
        .chain(config.overrides.iter().flat_map(|item| item.rules.keys()))
    {
        if all_rules().iter().all(|rule| rule.code != code) {
            let suggestion = all_rules()
                .iter()
                .min_by_key(|rule| levenshtein(code, rule.code))
                .map(|rule| rule.code);
            let suffix =
                suggestion.map_or(String::new(), |value| format!("; did you mean {value}?"));
            return Err(ConfigError::new(format!(
                "unknown rule code {code}{suffix}"
            )));
        }
    }
    for item in &config.overrides {
        if item.path.is_empty() {
            return Err(ConfigError::new("override.path must not be empty"));
        }
        if let Some(value) = &item.orthography {
            parse_orthography(value)?;
        }
    }
    Ok(())
}

fn parse_orthography(value: &str) -> Result<Orthography, ConfigError> {
    match value {
        "modern" => Ok(Orthography::Modern),
        "traditional" => Ok(Orthography::Traditional),
        "mixed" => Ok(Orthography::Mixed),
        _ => Err(ConfigError::new(format!(
            "invalid orthography {value:?}; expected modern, traditional, or mixed"
        ))),
    }
}

fn parse_severity(value: &str) -> Result<SeverityArg, ConfigError> {
    match value {
        "error" => Ok(SeverityArg::Error),
        "warning" => Ok(SeverityArg::Warning),
        "note" => Ok(SeverityArg::Note),
        _ => Err(ConfigError::new(format!("invalid fail-on value {value:?}"))),
    }
}

fn parse_format(value: &str) -> Result<Format, ConfigError> {
    match value {
        "auto" => Ok(Format::Auto),
        "human" => Ok(Format::Human),
        "json" => Ok(Format::Json),
        "short" => Ok(Format::Short),
        "sarif" => Ok(Format::Sarif),
        _ => Err(ConfigError::new(format!("invalid format {value:?}"))),
    }
}

fn parse_color(value: &str) -> Result<ColorChoice, ConfigError> {
    match value {
        "auto" => Ok(ColorChoice::Auto),
        "always" => Ok(ColorChoice::Always),
        "never" => Ok(ColorChoice::Never),
        _ => Err(ConfigError::new(format!("invalid color {value:?}"))),
    }
}

fn parse_language(value: &str) -> Result<LanguageArg, ConfigError> {
    match value {
        "en" | "en-US" | "en_US" => Ok(LanguageArg::En),
        "ja" | "ja-JP" | "ja_JP" => Ok(LanguageArg::Ja),
        _ => Err(ConfigError::new(format!("unsupported language {value:?}"))),
    }
}

fn language_from_lang() -> LanguageArg {
    env::var("LANG").map_or(LanguageArg::En, |value| {
        if value.to_ascii_lowercase().starts_with("ja") {
            LanguageArg::Ja
        } else {
            LanguageArg::En
        }
    })
}

const fn severity_name(value: SeverityArg) -> &'static str {
    match value {
        SeverityArg::Error => "error",
        SeverityArg::Warning => "warning",
        SeverityArg::Note => "note",
    }
}

const fn format_name(value: Format) -> &'static str {
    match value {
        Format::Auto => "auto",
        Format::Human => "human",
        Format::Json => "json",
        Format::Short => "short",
        Format::Sarif => "sarif",
    }
}

const fn color_name(value: ColorChoice) -> &'static str {
    match value {
        ColorChoice::Auto => "auto",
        ColorChoice::Always => "always",
        ColorChoice::Never => "never",
    }
}

const fn language_name(value: LanguageArg) -> &'static str {
    match value {
        LanguageArg::En => "en",
        LanguageArg::Ja => "ja",
    }
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    wildcard_bytes(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_bytes(pattern: &[u8], value: &[u8]) -> bool {
    match pattern.split_first() {
        None => value.is_empty(),
        Some((&b'*', rest)) => {
            wildcard_bytes(rest, value)
                || value
                    .split_first()
                    .is_some_and(|(_, tail)| wildcard_bytes(pattern, tail))
        }
        Some((&b'?', rest)) => value
            .split_first()
            .is_some_and(|(_, tail)| wildcard_bytes(rest, tail)),
        Some((&expected, rest)) => value
            .split_first()
            .is_some_and(|(&actual, tail)| expected == actual && wildcard_bytes(rest, tail)),
    }
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut costs: Vec<usize> = (0..=right.chars().count()).collect();
    for (row, left_char) in left.chars().enumerate() {
        let mut diagonal = row;
        if let Some(first) = costs.first_mut() {
            *first = row + 1;
        }
        for (column, right_char) in right.chars().enumerate() {
            let above = costs.get(column + 1).copied().unwrap_or(usize::MAX);
            let left_cost = costs.get(column).copied().unwrap_or(usize::MAX);
            let replacement = diagonal + usize::from(left_char != right_char);
            diagonal = above;
            if let Some(cell) = costs.get_mut(column + 1) {
                *cell = replacement.min(above + 1).min(left_cost + 1);
            }
        }
    }
    costs.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards_and_did_you_mean_distance_are_stable() {
        assert!(wildcard_match("src/*.txt", "src/work.txt"));
        assert!(!wildcard_match("src/*.txt", "test/work.txt"));
        assert!(
            levenshtein(
                "aozora::proof::encoding::line_endin",
                "aozora::proof::encoding::line_ending"
            ) < 3
        );
    }
}

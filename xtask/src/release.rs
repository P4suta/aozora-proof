//! Release version synchronization, qualification, and preflight checks.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

const MANIFEST_PATH: &str = ".release-please-manifest.json";
const RELEASE_CONFIG_PATH: &str = "release-please-config.json";
const VERSION_PATH: &str = "version.txt";
const REPORT_SCHEMA_VERSION: u32 = 2;
const RELEASE_TARGETS: [&str; 5] = [
    "aarch64-apple-darwin",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-musl",
];

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        let mut parts = input.split('.');
        let major = parse_version_number(parts.next(), input)?;
        let minor = parse_version_number(parts.next(), input)?;
        let patch = parse_version_number(parts.next(), input)?;
        if parts.next().is_some() {
            return Err(format!("{input:?} is not a stable X.Y.Z version"));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

fn parse_version_number(part: Option<&str>, whole: &str) -> Result<u64, String> {
    let part = part.ok_or_else(|| format!("{whole:?} is not a stable X.Y.Z version"))?;
    if part.is_empty() || (part.len() > 1 && part.starts_with('0')) {
        return Err(format!("{whole:?} is not a canonical X.Y.Z version"));
    }
    part.parse::<u64>()
        .map_err(|error| format!("{whole:?} is not a stable X.Y.Z version: {error}"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactManifest {
    schema_version: u32,
    commit: String,
    version: String,
    report_schema_version: u32,
    artifacts: Vec<Artifact>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Artifact {
    target: String,
    platform: String,
    archive: String,
    sha256: String,
    sbom: String,
}

#[allow(
    clippy::redundant_pub_crate,
    reason = "the private release module exposes one entry point to its parent"
)]
pub(crate) fn run(mut args: impl Iterator<Item = OsString>) -> Result<String, String> {
    let command = args
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| release_usage().to_owned())?;
    let root = workspace_root()?;
    match command.as_str() {
        "sync" => {
            reject_arguments(args)?;
            let version = read_version(&root)?;
            sync(&root, &version)?;
            check(&root, None)?;
            Ok(format!("release sync: {version}"))
        }
        "check" => {
            let tag = parse_optional_value(args, "--tag")?;
            let version = check(&root, tag.as_deref())?;
            Ok(format!("release check: {version}"))
        }
        "preflight" => {
            let options = parse_named_values(args)?;
            let repository = options
                .get("--repository")
                .cloned()
                .or_else(|| std::env::var("GITHUB_REPOSITORY").ok())
                .ok_or_else(|| "preflight requires --repository OWNER/REPO".to_owned())?;
            let commit = options
                .get("--commit")
                .cloned()
                .unwrap_or_else(|| "HEAD".to_owned());
            let version = check(&root, None)?;
            preflight(&repository, &commit)?;
            Ok(format!(
                "release preflight: {repository} {commit} is ready for v{version}"
            ))
        }
        "qualify" => {
            let options = parse_named_values(args)?;
            let event = required_option(&options, "--event")?;
            let reference = required_option(&options, "--ref")?;
            let changed = required_option(&options, "--version-changed")? == "true";
            let release_pr = options
                .get("--release-pr")
                .is_some_and(|value| value == "true");
            let full = should_qualify(
                event,
                reference,
                Qualification {
                    version_changed: changed,
                    release_pr,
                },
            );
            println!("full={full}");
            Ok(if full {
                "release qualification: full".to_owned()
            } else {
                "release qualification: no-op".to_owned()
            })
        }
        "artifact-check" => {
            let manifest = args
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "artifact-check requires a manifest path".to_owned())?;
            let options = parse_named_values(args)?;
            let commit = options.get("--commit").map(String::as_str);
            let version = options.get("--version").map(String::as_str);
            check_artifact_manifest(&manifest, commit, version, true)?;
            Ok(format!("release artifacts: {}", manifest.display()))
        }
        _ => Err(release_usage().to_owned()),
    }
}

const fn release_usage() -> &'static str {
    "usage: cargo xtask release <sync|check|preflight|qualify|artifact-check>"
}

fn workspace_root() -> Result<PathBuf, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "xtask manifest has no workspace parent".to_owned())
}

fn reject_arguments(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    if let Some(value) = args.next() {
        return Err(format!("unexpected argument: {}", value.to_string_lossy()));
    }
    Ok(())
}

fn parse_optional_value(
    mut args: impl Iterator<Item = OsString>,
    name: &str,
) -> Result<Option<String>, String> {
    let Some(argument) = args.next() else {
        return Ok(None);
    };
    if argument != name {
        return Err(format!(
            "expected {name}, got {}",
            argument.to_string_lossy()
        ));
    }
    let value = args
        .next()
        .ok_or_else(|| format!("{name} requires a value"))?
        .into_string()
        .map_err(|argument| format!("{name} must be UTF-8, got {}", argument.display()))?;
    reject_arguments(args)?;
    Ok(Some(value))
}

fn parse_named_values(
    mut args: impl Iterator<Item = OsString>,
) -> Result<BTreeMap<String, String>, String> {
    let mut values = BTreeMap::new();
    while let Some(name) = args.next() {
        let name = name.into_string().map_err(|argument| {
            format!("option names must be UTF-8, got {}", argument.display())
        })?;
        if !name.starts_with("--") {
            return Err(format!("expected an option, got {name}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{name} requires a value"))?
            .into_string()
            .map_err(|argument| format!("{name} must be UTF-8, got {}", argument.display()))?;
        if values.insert(name.clone(), value).is_some() {
            return Err(format!("{name} was provided more than once"));
        }
    }
    Ok(values)
}

fn required_option<'a>(
    options: &'a BTreeMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    options
        .get(name)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} is required"))
}

fn read_version(root: &Path) -> Result<Version, String> {
    let contents = read(root.join(VERSION_PATH))?;
    Version::parse(&contents)
}

fn read(path: impl AsRef<Path>) -> Result<String, String> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    if read(path)? == contents {
        return Ok(());
    }
    fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

fn sync(root: &Path, version: &Version) -> Result<(), String> {
    let version = version.to_string();
    let cargo_path = root.join("Cargo.toml");
    let cargo = read(&cargo_path)?;
    let cargo = replace_workspace_version(&cargo, &version)?;
    write_if_changed(&cargo_path, &cargo)?;

    let readme_path = root.join("README.md");
    let readme = read(&readme_path)?;
    let readme = replace_action_references(&readme, &version)?;
    write_if_changed(&readme_path, &readme)?;

    for relative in ["Cargo.lock", "fuzz/Cargo.lock"] {
        let path = root.join(relative);
        let lock = read(&path)?;
        let lock = replace_lock_versions(&lock, &version)?;
        write_if_changed(&path, &lock)?;
    }
    Ok(())
}

fn replace_workspace_version(contents: &str, version: &str) -> Result<String, String> {
    let mut result = String::with_capacity(contents.len());
    let mut in_workspace_package = false;
    let mut workspace_version_seen = false;
    let mut internal_dependencies = 0_u8;
    for line in contents.lines() {
        if line.starts_with('[') {
            in_workspace_package = line == "[workspace.package]";
        }
        let replacement = if in_workspace_package && line.trim_start().starts_with("version") {
            workspace_version_seen = true;
            format!("version       = \"{version}\"")
        } else if line.starts_with("aozora-proof-core = { version = ") {
            internal_dependencies = internal_dependencies.saturating_add(1);
            format!(
                "aozora-proof-core = {{ version = \"{version}\", path = \"crates/aozora-proof-core\" }}"
            )
        } else if line.starts_with("aozora-proof-data = { version = ") {
            internal_dependencies = internal_dependencies.saturating_add(1);
            format!(
                "aozora-proof-data = {{ version = \"{version}\", path = \"crates/aozora-proof-data\" }}"
            )
        } else {
            line.to_owned()
        };
        result.push_str(&replacement);
        result.push('\n');
    }
    if !workspace_version_seen || internal_dependencies != 2 {
        return Err("Cargo.toml does not contain the expected release version fields".to_owned());
    }
    Ok(result)
}

fn replace_action_references(contents: &str, version: &str) -> Result<String, String> {
    let marker = "P4suta/aozora-proof/action@v";
    let mut replacements = 0_u32;
    let mut result = String::with_capacity(contents.len());
    for line in contents.lines() {
        if let Some(start) = line.find(marker) {
            let value_start = start
                .checked_add(marker.len())
                .ok_or_else(|| "README Action reference offset overflowed".to_owned())?;
            let suffix = line
                .get(value_start..)
                .ok_or_else(|| "invalid README Action reference".to_owned())?;
            let value_end = suffix
                .find(|character: char| !(character.is_ascii_digit() || character == '.'))
                .map_or(Ok(line.len()), |offset| {
                    value_start
                        .checked_add(offset)
                        .ok_or_else(|| "README Action reference offset overflowed".to_owned())
                })?;
            result.push_str(
                line.get(..value_start)
                    .ok_or_else(|| "invalid README action reference".to_owned())?,
            );
            result.push_str(version);
            result.push_str(
                line.get(value_end..)
                    .ok_or_else(|| "invalid README action reference".to_owned())?,
            );
            replacements = replacements.saturating_add(1);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    if replacements == 0 {
        return Err("README.md has no versioned aozora-proof Action reference".to_owned());
    }
    Ok(result)
}

fn replace_lock_versions(contents: &str, version: &str) -> Result<String, String> {
    let names = [
        "aozora-proof-cli",
        "aozora-proof-core",
        "aozora-proof-data",
        "aozora-proof-wasm",
    ];
    let mut result = String::with_capacity(contents.len());
    let mut package_is_internal = false;
    let mut replacements = 0_u32;
    for line in contents.lines() {
        if line == "[[package]]" {
            package_is_internal = false;
        }
        if let Some(name) = line
            .strip_prefix("name = \"")
            .and_then(|value| value.strip_suffix('"'))
        {
            package_is_internal = names.contains(&name);
        }
        if package_is_internal && line.starts_with("version = \"") {
            writeln!(result, "version = \"{version}\"")
                .map_err(|error| format!("could not update lockfile: {error}"))?;
            replacements = replacements.saturating_add(1);
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    if replacements == 0 {
        return Err("lockfile contains no versioned internal package".to_owned());
    }
    Ok(result)
}

fn check(root: &Path, explicit_tag: Option<&str>) -> Result<Version, String> {
    let version = read_version(root)?;
    check_release_please(root, &version)?;
    check_cargo(root, &version)?;
    check_lock(root, &version)?;
    check_readme(root, &version)?;
    check_changelog(root, &version)?;
    check_ruleset_files(root)?;
    check_action_pins(root)?;
    check_tool_locks(root)?;
    check_git(root, &version, explicit_tag)?;
    Ok(version)
}

fn check_release_please(root: &Path, version: &Version) -> Result<(), String> {
    let manifest: Value = serde_json::from_str(&read(root.join(MANIFEST_PATH))?)
        .map_err(|error| format!("{MANIFEST_PATH}: {error}"))?;
    if manifest.get(".").and_then(Value::as_str) != Some(version.to_string().as_str()) {
        return Err(format!("{MANIFEST_PATH} does not match {VERSION_PATH}"));
    }

    let config: Value = serde_json::from_str(&read(root.join(RELEASE_CONFIG_PATH))?)
        .map_err(|error| format!("{RELEASE_CONFIG_PATH}: {error}"))?;
    let bootstrap = config
        .get("bootstrap-sha")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{RELEASE_CONFIG_PATH}: bootstrap-sha is required"))?;
    if !is_lower_hex(bootstrap, 40) {
        return Err(format!(
            "{RELEASE_CONFIG_PATH}: bootstrap-sha must be a full lowercase SHA-1"
        ));
    }
    if root.join(".git").exists()
        && git(root, &["merge-base", "--is-ancestor", bootstrap, "HEAD"]).is_err()
    {
        return Err(format!(
            "{RELEASE_CONFIG_PATH}: bootstrap-sha {bootstrap} is not an ancestor of HEAD"
        ));
    }
    let package = config
        .get("packages")
        .and_then(|packages| packages.get("."))
        .ok_or_else(|| format!("{RELEASE_CONFIG_PATH} must configure root package ."))?;
    for (field, expected) in [("release-type", "simple"), ("component", "aozora-proof")] {
        if package.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(format!("{RELEASE_CONFIG_PATH}: {field} must be {expected}"));
        }
    }
    for field in [
        "skip-github-release",
        "bump-minor-pre-major",
        "bump-patch-for-minor-pre-major",
    ] {
        if package.get(field).and_then(Value::as_bool) != Some(true) {
            return Err(format!("{RELEASE_CONFIG_PATH}: {field} must be true"));
        }
    }
    Ok(())
}

fn check_cargo(root: &Path, version: &Version) -> Result<(), String> {
    let cargo = read(root.join("Cargo.toml"))?;
    let expected = replace_workspace_version(&cargo, &version.to_string())?;
    if cargo != expected {
        return Err(
            "Cargo.toml release versions are out of sync; run just release-sync".to_owned(),
        );
    }
    Ok(())
}

fn check_lock(root: &Path, version: &Version) -> Result<(), String> {
    for relative in ["Cargo.lock", "fuzz/Cargo.lock"] {
        let lock = read(root.join(relative))?;
        let expected = replace_lock_versions(&lock, &version.to_string())?;
        if lock != expected {
            return Err(format!(
                "{relative} internal package versions are out of sync; run just release-sync"
            ));
        }
    }
    Ok(())
}

fn check_readme(root: &Path, version: &Version) -> Result<(), String> {
    let readme = read(root.join("README.md"))?;
    let expected = replace_action_references(&readme, &version.to_string())?;
    if readme != expected {
        return Err("README.md Action reference is out of sync; run just release-sync".to_owned());
    }
    Ok(())
}

fn check_changelog(root: &Path, version: &Version) -> Result<(), String> {
    let changelog = read(root.join("CHANGELOG.md"))?;
    let first_version = changelog
        .lines()
        .find_map(|line| line.strip_prefix("## ["))
        .and_then(|line| line.split(']').next())
        .ok_or_else(|| "CHANGELOG.md has no release heading".to_owned())?;
    if first_version != version.to_string() {
        return Err(format!(
            "CHANGELOG.md first release is {first_version}, expected {version}"
        ));
    }
    if changelog.contains("## [Unreleased]") {
        return Err(
            "CHANGELOG.md must be managed by Release Please without [Unreleased]".to_owned(),
        );
    }
    Ok(())
}

fn check_ruleset_files(root: &Path) -> Result<(), String> {
    let main_path = root.join(".github/rulesets/main.json");
    let main: Value = serde_json::from_str(&read(&main_path)?)
        .map_err(|error| format!("{}: {error}", main_path.display()))?;
    if main.get("name").and_then(Value::as_str) != Some("main")
        || main.get("enforcement").and_then(Value::as_str) != Some("active")
    {
        return Err("main ruleset must be named main and active".to_owned());
    }
    check_main_ruleset(&main)?;

    let tags_path = root.join(".github/rulesets/tags.json");
    let tags: Value = serde_json::from_str(&read(&tags_path)?)
        .map_err(|error| format!("{}: {error}", tags_path.display()))?;
    if tags.get("name").and_then(Value::as_str) != Some("v*")
        || tags.get("enforcement").and_then(Value::as_str) != Some("active")
    {
        return Err("tag ruleset must be named v* and active".to_owned());
    }
    check_tag_ruleset(&tags)?;
    Ok(())
}

fn check_action_pins(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect_yaml_files(&root.join(".github"), &mut files)
        .map_err(|error| format!(".github: {error}"))?;
    files.push(root.join("action/action.yml"));
    for path in files {
        for (index, line) in read(&path)?.lines().enumerate() {
            let line = line
                .trim_start()
                .strip_prefix("- ")
                .unwrap_or_else(|| line.trim_start());
            let Some(reference) = line.strip_prefix("uses:") else {
                continue;
            };
            let reference = reference.split_whitespace().next().unwrap_or("");
            if reference.starts_with("./") {
                continue;
            }
            let revision = reference
                .rsplit_once('@')
                .map(|(_, revision)| revision)
                .ok_or_else(|| {
                    format!(
                        "{}:{} Action has no revision",
                        path.display(),
                        index.saturating_add(1)
                    )
                })?;
            if !is_lower_hex(revision, 40) {
                return Err(format!(
                    "{}:{} third-party Action is not pinned to a full commit SHA",
                    path.display(),
                    index.saturating_add(1)
                ));
            }
        }
    }
    Ok(())
}

fn collect_yaml_files(directory: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_yaml_files(&path, files)?;
        } else if path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| matches!(extension, "yml" | "yaml"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn check_tool_locks(root: &Path) -> Result<(), String> {
    for (config_relative, lock_relative) in [
        ("mise.toml", "mise.lock"),
        (".config/mise/config.toml", ".config/mise/mise.lock"),
    ] {
        let lock = read(root.join(lock_relative))?;
        if !lock.contains("# @generated") || !lock.contains("version = ") {
            return Err(format!("{lock_relative} is not a generated mise lockfile"));
        }
        let config = read(root.join(config_relative))?;
        if config.contains("\"latest\"") {
            return Err(format!(
                "{config_relative} contains an unpinned latest tool"
            ));
        }
        for line in config.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let version = line
                .split_once('=')
                .map(|(_, version)| version.trim().trim_matches('"'))
                .ok_or_else(|| format!("{config_relative} has an invalid tool line"))?;
            let concrete = version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
                && version.bytes().filter(|byte| *byte == b'.').count() >= 2;
            if !concrete {
                return Err(format!(
                    "{config_relative} tool version {version:?} is not concrete"
                ));
            }
            if !lock.contains(&format!("version = \"{version}\"")) {
                return Err(format!(
                    "{lock_relative} does not lock configured version {version}"
                ));
            }
        }
    }
    Ok(())
}

fn check_git(root: &Path, version: &Version, explicit_tag: Option<&str>) -> Result<(), String> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    let tags = git(root, &["tag", "--list", "v[0-9]*.[0-9]*.[0-9]*"])?;
    let mut versions = Vec::new();
    for tag in tags.lines() {
        if let Some(raw) = tag.strip_prefix('v') {
            versions.push(Version::parse(raw)?);
        }
    }
    ensure_monotonic(version, &versions)?;

    let environment_tag = if std::env::var("GITHUB_REF_TYPE").as_deref() == Ok("tag") {
        std::env::var("GITHUB_REF_NAME").ok()
    } else {
        None
    };
    let tag = explicit_tag.map(str::to_owned).or(environment_tag);
    if let Some(tag) = tag {
        validate_tag_name(version, &tag)?;
        let tagged = git(root, &["rev-list", "-n", "1", &tag])?;
        let head = git(root, &["rev-parse", "HEAD"])?;
        if tagged.trim() != head.trim() {
            return Err(format!("tag {tag} does not point to HEAD"));
        }
    }
    Ok(())
}

fn ensure_monotonic(version: &Version, released: &[Version]) -> Result<(), String> {
    if let Some(latest) = released.iter().max()
        && version < latest
    {
        return Err(format!(
            "version {version} is behind existing tag v{latest}"
        ));
    }
    Ok(())
}

fn validate_tag_name(version: &Version, tag: &str) -> Result<(), String> {
    let expected = format!("v{version}");
    if tag != expected {
        return Err(format!("tag {tag} does not match version {version}"));
    }
    Ok(())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("git output was not UTF-8: {error}"))
}

#[derive(Clone, Copy, Debug)]
struct Qualification {
    version_changed: bool,
    release_pr: bool,
}

fn should_qualify(event: &str, reference: &str, qualification: Qualification) -> bool {
    qualification.version_changed
        && match event {
            "push" => reference == "refs/heads/main",
            "pull_request" => qualification.release_pr,
            _ => false,
        }
}

fn check_artifact_manifest(
    path: &Path,
    expected_commit: Option<&str>,
    expected_version: Option<&str>,
    check_files: bool,
) -> Result<(), String> {
    let manifest: ArtifactManifest = serde_json::from_str(&read(path)?)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    if manifest.schema_version != 1 {
        return Err("artifact manifest schemaVersion must be 1".to_owned());
    }
    if !is_lower_hex(&manifest.commit, 40) {
        return Err("artifact manifest commit must be a full lowercase SHA-1".to_owned());
    }
    if expected_commit.is_some_and(|expected| expected != manifest.commit) {
        return Err("artifact manifest commit does not match requested commit".to_owned());
    }
    Version::parse(&manifest.version)?;
    if expected_version.is_some_and(|expected| expected != manifest.version) {
        return Err("artifact manifest version does not match requested version".to_owned());
    }
    if manifest.report_schema_version != REPORT_SCHEMA_VERSION {
        return Err(format!(
            "artifact manifest reportSchemaVersion must remain {REPORT_SCHEMA_VERSION}"
        ));
    }

    let targets = manifest
        .artifacts
        .iter()
        .map(|artifact| artifact.target.as_str())
        .collect::<BTreeSet<_>>();
    let expected = RELEASE_TARGETS.into_iter().collect::<BTreeSet<_>>();
    if targets != expected || manifest.artifacts.len() != expected.len() {
        return Err("artifact manifest must contain each supported target exactly once".to_owned());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for artifact in &manifest.artifacts {
        if artifact.platform.trim().is_empty() {
            return Err(format!("{} has an empty platform", artifact.target));
        }
        check_relative_asset_name(&artifact.archive)?;
        check_relative_asset_name(&artifact.sbom)?;
        if !is_lower_hex(&artifact.sha256, 64) {
            return Err(format!("{} has an invalid SHA-256", artifact.archive));
        }
        if check_files {
            let archive = parent.join(&artifact.archive);
            let actual = sha256(&archive)?;
            if actual != artifact.sha256 {
                return Err(format!(
                    "{} SHA-256 does not match manifest",
                    archive.display()
                ));
            }
            let sbom = parent.join(&artifact.sbom);
            if !sbom.is_file() {
                return Err(format!("{} is missing", sbom.display()));
            }
        }
    }
    Ok(())
}

fn check_relative_asset_name(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_)) || path.components().count() != 1
        })
    {
        return Err(format!("{value:?} is not a safe release asset name"));
    }
    Ok(())
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256(path: &Path) -> Result<String, String> {
    let commands: [(&str, &[&str]); 2] = [("sha256sum", &[]), ("shasum", &["--algorithm", "256"])];
    for (program, arguments) in commands {
        let output = Command::new(program).args(arguments).arg(path).output();
        let Ok(output) = output else {
            continue;
        };
        if output.status.success() {
            let text = String::from_utf8(output.stdout)
                .map_err(|error| format!("{program} output was not UTF-8: {error}"))?;
            return text
                .split_whitespace()
                .next()
                .map(str::to_owned)
                .ok_or_else(|| format!("{program} returned no digest"));
        }
    }
    Err("neither sha256sum nor shasum could hash the release asset".to_owned())
}

fn preflight(repository: &str, commit: &str) -> Result<(), String> {
    if repository.split('/').count() != 2 {
        return Err("--repository must be OWNER/REPO".to_owned());
    }
    let repository_info = gh_json(&format!("repos/{repository}"))?;
    let repository_id = repository_info
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| "GitHub repository response has no numeric id".to_owned())?;

    let release_please = gh_json(&format!("repos/{repository}/environments/release-please"))?;
    check_environment_branch_policy(&release_please, "main")?;
    check_environment_pattern(repository, "release-please", "main")?;
    check_no_required_reviewers(&release_please, "release-please")?;
    let release = gh_json(&format!("repos/{repository}/environments/release"))?;
    check_environment_branch_policy(&release, "v*")?;
    check_environment_pattern(repository, "release", "v*")?;
    check_release_reviewers(&release)?;

    let secrets = gh_json(&format!(
        "repositories/{repository_id}/environments/release-please/secrets"
    ))?;
    let secret_names = secrets
        .get("secrets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|secret| secret.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in ["RELEASE_PLEASE_APP_ID", "RELEASE_PLEASE_APP_PRIVATE_KEY"] {
        if !secret_names.contains(required) {
            return Err(format!("release-please Environment is missing {required}"));
        }
    }
    let release_secrets = gh_json(&format!(
        "repositories/{repository_id}/environments/release/secrets"
    ))?;
    if release_secrets.get("total_count").and_then(Value::as_u64) != Some(0) {
        return Err("release Environment must not store publish credentials".to_owned());
    }

    let rulesets = gh_json(&format!("repos/{repository}/rulesets"))?;
    let main_ruleset = active_ruleset(repository, &rulesets, "main")?;
    check_main_ruleset(&main_ruleset)?;
    let tag_ruleset = active_ruleset(repository, &rulesets, "v*")?;
    check_tag_ruleset(&tag_ruleset)?;

    let immutable = gh_json_with_headers(
        &format!("repos/{repository}/immutable-releases"),
        &["X-GitHub-Api-Version: 2026-03-10"],
    )?;
    if immutable.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err("GitHub release immutability is not enabled".to_owned());
    }

    let resolved_commit = if commit == "HEAD" {
        git(Path::new("."), &["rev-parse", "HEAD"])?
            .trim()
            .to_owned()
    } else {
        commit.to_owned()
    };
    if !is_lower_hex(&resolved_commit, 40) {
        return Err("preflight commit must be a full lowercase SHA-1 or HEAD".to_owned());
    }
    let runs = gh_json(&format!(
        "repos/{repository}/actions/workflows/release-ready.yml/runs?head_sha={resolved_commit}&status=success&event=push&per_page=1"
    ))?;
    if runs.get("total_count").and_then(Value::as_u64) == Some(0) {
        return Err(format!(
            "no successful release-ready push run exists for {resolved_commit}"
        ));
    }
    Ok(())
}

fn check_environment_pattern(
    repository: &str,
    environment: &str,
    expected_pattern: &str,
) -> Result<(), String> {
    let policies = gh_json(&format!(
        "repos/{repository}/environments/{environment}/deployment-branch-policies"
    ))?;
    let patterns = policies
        .get("branch_policies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|policy| policy.get("name").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if patterns.as_slice() != [expected_pattern] {
        return Err(format!(
            "{environment} Environment must allow only {expected_pattern}, found {patterns:?}"
        ));
    }
    Ok(())
}

fn check_environment_branch_policy(
    environment: &Value,
    expected_pattern: &str,
) -> Result<(), String> {
    let policies = environment
        .get("deployment_branch_policy")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Environment must restrict deployments to {expected_pattern}"))?;
    if policies
        .get("custom_branch_policies")
        .and_then(Value::as_bool)
        != Some(true)
        || policies.get("protected_branches").and_then(Value::as_bool) != Some(false)
    {
        return Err(format!(
            "Environment must use custom branch policy {expected_pattern}"
        ));
    }
    Ok(())
}

fn check_no_required_reviewers(environment: &Value, name: &str) -> Result<(), String> {
    let has_reviewers = environment
        .get("protection_rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|rule| rule.get("type").and_then(Value::as_str) == Some("required_reviewers"));
    if has_reviewers {
        return Err(format!("{name} Environment must not require a reviewer"));
    }
    Ok(())
}

fn check_release_reviewers(environment: &Value) -> Result<(), String> {
    let protection_rules = environment
        .get("protection_rules")
        .and_then(Value::as_array)
        .ok_or_else(|| "release Environment has no protection rules".to_owned())?;
    let reviewers = protection_rules
        .iter()
        .find(|rule| rule.get("type").and_then(Value::as_str) == Some("required_reviewers"))
        .and_then(|rule| rule.get("reviewers"))
        .and_then(Value::as_array)
        .ok_or_else(|| "release Environment requires reviewer P4suta".to_owned())?;
    let has_maintainer = reviewers.iter().any(|reviewer| {
        reviewer
            .get("reviewer")
            .and_then(|value| value.get("login"))
            .and_then(Value::as_str)
            == Some("P4suta")
    });
    if !has_maintainer {
        return Err("release Environment requires reviewer P4suta".to_owned());
    }
    let prevents_self_review = protection_rules
        .iter()
        .any(|rule| rule.get("prevent_self_review").and_then(Value::as_bool) == Some(true));
    if prevents_self_review {
        return Err("release Environment must allow solo-maintainer self-review".to_owned());
    }
    Ok(())
}

fn active_ruleset(repository: &str, rulesets: &Value, name: &str) -> Result<Value, String> {
    let id = rulesets
        .as_array()
        .into_iter()
        .flatten()
        .find(|ruleset| {
            ruleset.get("name").and_then(Value::as_str) == Some(name)
                && ruleset.get("enforcement").and_then(Value::as_str) == Some("active")
        })
        .and_then(|ruleset| ruleset.get("id"))
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("active GitHub ruleset {name:?} is missing"))?;
    gh_json(&format!("repos/{repository}/rulesets/{id}"))
}

fn check_main_ruleset(ruleset: &Value) -> Result<(), String> {
    let rules = ruleset
        .get("rules")
        .and_then(Value::as_array)
        .ok_or_else(|| "main ruleset has no rules".to_owned())?;
    let types = rules
        .iter()
        .filter_map(|rule| rule.get("type").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in [
        "deletion",
        "non_fast_forward",
        "required_linear_history",
        "required_signatures",
        "pull_request",
        "required_status_checks",
    ] {
        if !types.contains(required) {
            return Err(format!("main ruleset is missing {required}"));
        }
    }
    let pull_request = rules
        .iter()
        .find(|rule| rule.get("type").and_then(Value::as_str) == Some("pull_request"))
        .and_then(|rule| rule.get("parameters"))
        .ok_or_else(|| "main pull_request rule has no parameters".to_owned())?;
    let squash_only = pull_request
        .get("allowed_merge_methods")
        .and_then(Value::as_array)
        .is_some_and(|methods| {
            methods.len() == 1 && methods.first().and_then(Value::as_str) == Some("squash")
        });
    if !squash_only
        || pull_request
            .get("required_review_thread_resolution")
            .and_then(Value::as_bool)
            != Some(true)
    {
        return Err("main ruleset must require squash PRs and conversation resolution".to_owned());
    }
    let status = rules
        .iter()
        .find(|rule| rule.get("type").and_then(Value::as_str) == Some("required_status_checks"))
        .and_then(|rule| rule.get("parameters"))
        .ok_or_else(|| "main status-check rule has no parameters".to_owned())?;
    if status
        .get("strict_required_status_checks_policy")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("main status checks must be strict".to_owned());
    }
    let contexts = status
        .get("required_status_checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|check| check.get("context").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in ["ci-success", "release-ready", "codeql", "dependency-review"] {
        if !contexts.contains(required) {
            return Err(format!("main ruleset is missing required check {required}"));
        }
    }
    Ok(())
}

fn check_tag_ruleset(ruleset: &Value) -> Result<(), String> {
    let types = ruleset
        .get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|rule| rule.get("type").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in ["update", "deletion", "non_fast_forward"] {
        if !types.contains(required) {
            return Err(format!("v* ruleset is missing {required}"));
        }
    }
    Ok(())
}

fn gh_json(endpoint: &str) -> Result<Value, String> {
    gh_json_with_headers(endpoint, &[])
}

fn gh_json_with_headers(endpoint: &str, headers: &[&str]) -> Result<Value, String> {
    let mut command = Command::new("gh");
    command.args(["api", endpoint]);
    for header in headers {
        command.args(["-H", header]);
    }
    let output = command
        .output()
        .map_err(|error| format!("could not run gh: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh api {endpoint} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("gh api {endpoint} returned invalid JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_ARTIFACTS: &str = include_str!("../tests/fixtures/release/artifacts-valid.json");
    const INVALID_ARTIFACTS: &str =
        include_str!("../tests/fixtures/release/artifacts-invalid-target.json");

    #[test]
    fn initial_and_future_versions_are_monotonic() {
        let initial = Version::parse("0.1.0").expect("initial version");
        let patch = Version::parse("0.1.1").expect("patch version");
        let minor = Version::parse("0.2.0").expect("minor version");
        assert!(initial < patch);
        assert!(patch < minor);
    }

    #[test]
    fn version_regression_is_detected() {
        let released = Version::parse("0.2.0").expect("released version");
        let candidate = Version::parse("0.1.9").expect("candidate version");
        ensure_monotonic(&candidate, &[released]).expect_err("regression must fail");
    }

    #[test]
    fn tags_must_match_versions() {
        let version = Version::parse("0.1.0").expect("release version");
        validate_tag_name(&version, "v0.1.0").expect("matching tag");
        validate_tag_name(&version, "v0.1.1").expect_err("mismatched tag must fail");
    }

    #[test]
    fn lockfile_sync_only_changes_internal_packages() {
        let input = "[[package]]\nname = \"aozora-proof-core\"\nversion = \"0.1.0\"\n\n[[package]]\nname = \"other\"\nversion = \"0.1.0\"\n";
        let actual = replace_lock_versions(input, "0.1.1").expect("lockfile sync");
        assert!(actual.contains("name = \"aozora-proof-core\"\nversion = \"0.1.1\""));
        assert!(actual.contains("name = \"other\"\nversion = \"0.1.0\""));
    }

    #[test]
    fn artifact_manifest_contract_is_validated() {
        let valid: ArtifactManifest =
            serde_json::from_str(VALID_ARTIFACTS).expect("valid artifact fixture");
        validate_artifact_fixture(&valid).expect("valid artifact contract");
        let invalid: ArtifactManifest =
            serde_json::from_str(INVALID_ARTIFACTS).expect("invalid artifact fixture parses");
        validate_artifact_fixture(&invalid).expect_err("invalid target must fail");
    }

    #[test]
    fn ordinary_push_is_an_explicit_noop() {
        assert!(!should_qualify(
            "push",
            "refs/heads/main",
            Qualification {
                version_changed: false,
                release_pr: false,
            }
        ));
        assert!(!should_qualify(
            "pull_request",
            "refs/pull/7/merge",
            Qualification {
                version_changed: true,
                release_pr: false,
            }
        ));
        assert!(should_qualify(
            "pull_request",
            "refs/pull/7/merge",
            Qualification {
                version_changed: true,
                release_pr: true,
            }
        ));
    }

    fn validate_artifact_fixture(manifest: &ArtifactManifest) -> Result<(), String> {
        if manifest.schema_version != 1
            || manifest.report_schema_version != REPORT_SCHEMA_VERSION
            || !is_lower_hex(&manifest.commit, 40)
        {
            return Err("invalid fixture metadata".to_owned());
        }
        Version::parse(&manifest.version)?;
        let targets = manifest
            .artifacts
            .iter()
            .map(|artifact| artifact.target.as_str())
            .collect::<BTreeSet<_>>();
        let expected = RELEASE_TARGETS.into_iter().collect::<BTreeSet<_>>();
        if targets != expected || manifest.artifacts.len() != expected.len() {
            return Err("invalid fixture targets".to_owned());
        }
        for artifact in &manifest.artifacts {
            check_relative_asset_name(&artifact.archive)?;
            check_relative_asset_name(&artifact.sbom)?;
            if !is_lower_hex(&artifact.sha256, 64) {
                return Err("invalid fixture digest".to_owned());
            }
        }
        Ok(())
    }
}

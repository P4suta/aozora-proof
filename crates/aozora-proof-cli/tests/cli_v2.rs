//! Process-level contract tests for the initial 0.1 command surface.

#![allow(
    clippy::panic_in_result_fn,
    reason = "process contract tests propagate setup failures and use assertions for contract failures"
)]

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Output, Stdio};

use tempfile::tempdir;

#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

fn run(arguments: &[&str], input: &[u8], config_home: &Path) -> Result<Output, TestError> {
    run_with_environment(arguments, input, config_home, &[])
}

fn run_with_environment(
    arguments: &[&str],
    input: &[u8],
    config_home: &Path,
    environment: &[(&str, &str)],
) -> Result<Output, TestError> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_aozora-proof"));
    command
        .args(arguments)
        .env("XDG_CONFIG_HOME", config_home)
        .env_remove("AOZORA_PROOF_ORTHOGRAPHY")
        .env_remove("AOZORA_PROOF_FAIL_ON")
        .env_remove("AOZORA_PROOF_FORMAT")
        .env_remove("AOZORA_PROOF_COLOR")
        .env_remove("AOZORA_PROOF_LANG")
        .envs(environment.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("child stdin unavailable"))?
        .write_all(input);
    if write_result
        .as_ref()
        .is_err_and(|source| source.kind() != io::ErrorKind::BrokenPipe)
    {
        write_result?;
    }
    Ok(child.wait_with_output()?)
}

#[test]
fn help_and_version_expose_the_v2_commands() -> Result<(), TestError> {
    let config = tempdir()?;
    let help = run(&["--help"], b"", config.path())?;
    let version = run(&["--version"], b"", config.path())?;
    let help_text = String::from_utf8(help.stdout)?;
    let version_text = String::from_utf8(version.stdout)?;

    assert!(help.status.success());
    for command in [
        "check",
        "fix",
        "review",
        "explain",
        "gaiji",
        "rules",
        "init",
        "config",
        "completions",
        "man",
    ] {
        assert!(help_text.contains(command));
    }
    assert!(version_text.starts_with(concat!("aozora-proof ", env!("CARGO_PKG_VERSION"), " (")));
    Ok(())
}

#[test]
fn piped_document_requires_orthography() -> Result<(), TestError> {
    let config = tempdir()?;
    let output = run(
        &["check", "--no-input", "-"],
        "青空".as_bytes(),
        config.path(),
    )?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("--orthography modern|traditional|mixed"));
    Ok(())
}

#[test]
fn decode_failure_is_exit_two_without_machine_output() -> Result<(), TestError> {
    let config = tempdir()?;
    let output = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            "-",
        ],
        &[0xFF, 0xFF, 0xFF],
        config.path(),
    )?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)?.contains("cannot be decoded"));
    Ok(())
}

#[test]
fn json_is_schema_v2_and_language_invariant() -> Result<(), TestError> {
    let config = tempdir()?;
    let english = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            "--lang",
            "en",
            "-",
        ],
        "ｱ\n".as_bytes(),
        config.path(),
    )?;
    let japanese = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            "--lang",
            "ja",
            "-",
        ],
        "ｱ\n".as_bytes(),
        config.path(),
    )?;

    assert_eq!(english.status.code(), Some(1));
    assert_eq!(english.stdout, japanese.stdout);
    let value: serde_json::Value = serde_json::from_slice(&english.stdout)?;
    assert_eq!(
        value
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert!(value.get("tool").is_some());
    assert!(value.get("summary").is_some());
    assert!(value.get("files").is_some());
    Ok(())
}

#[test]
fn successful_schema_v2_output_is_byte_stable() -> Result<(), TestError> {
    let config = tempdir()?;
    let output = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            "-",
        ],
        b"",
        config.path(),
    )?;

    assert!(output.status.success());
    let expected = include_str!("fixtures/schema-v2-empty.json")
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
    assert_eq!(output.stdout, expected.as_bytes());
    Ok(())
}

#[test]
fn sarif_declares_unicode_columns_encoding_authority_and_fix() -> Result<(), TestError> {
    let config = tempdir()?;
    let output = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "sarif",
            "-",
        ],
        "ｱ\n".as_bytes(),
        config.path(),
    )?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let run = value
        .get("runs")
        .and_then(serde_json::Value::as_array)
        .and_then(|runs| runs.first())
        .ok_or_else(|| io::Error::other("SARIF run unavailable"))?;

    assert_eq!(
        value.get("version").and_then(serde_json::Value::as_str),
        Some("2.1.0")
    );
    assert_eq!(
        run.get("columnKind").and_then(serde_json::Value::as_str),
        Some("unicodeCodePoints")
    );
    assert!(
        run.get("artifacts")
            .and_then(serde_json::Value::as_array)
            .and_then(|artifacts| artifacts.first())
            .and_then(|artifact| artifact.get("encoding"))
            .is_some()
    );
    assert!(
        run.pointer("/tool/driver/rules/0/helpUri")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|uri| uri.starts_with("https://"))
    );
    assert!(
        run.get("results")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|results| results.iter().any(|result| {
                result
                    .get("fixes")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|fixes| !fixes.is_empty())
            }))
    );
    Ok(())
}

#[test]
fn stdin_fix_is_idempotent_shift_jis_output() -> Result<(), TestError> {
    let config = tempdir()?;
    let arguments = ["fix", "--orthography", "mixed", "--no-input", "-"];
    let first = run(&arguments, "ｶﾞ\n".as_bytes(), config.path())?;
    let second = run(&arguments, &first.stdout, config.path())?;

    assert!(first.status.success());
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    assert!(!first.stdout.is_empty());
    assert_ne!(first.stdout, "ガ\r\n".as_bytes());
    Ok(())
}

#[test]
fn directory_discovery_is_sorted_and_respects_ignore() -> Result<(), TestError> {
    let directory = tempdir()?;
    let config = tempdir()?;
    fs::write(directory.path().join("b.txt"), b"b\r\n")?;
    fs::write(directory.path().join("a.txt"), b"a\r\n")?;
    fs::write(directory.path().join("ignored.txt"), b"ignored\r\n")?;
    fs::write(directory.path().join(".hidden.txt"), b"hidden\r\n")?;
    fs::write(
        directory.path().join(".aozora-proofignore"),
        "ignored.txt\n",
    )?;
    let path = directory.path().to_string_lossy();
    let output = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            &path,
        ],
        b"",
        config.path(),
    )?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let files = value
        .get("files")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| io::Error::other("files array unavailable"))?;
    let paths: Vec<_> = files
        .iter()
        .filter_map(|file| file.get("path"))
        .filter_map(serde_json::Value::as_str)
        .collect();

    assert_eq!(paths.len(), 2);
    assert!(paths.windows(2).all(|pair| pair.first() <= pair.get(1)));

    let ignored = directory.path().join("ignored.txt");
    let ignored_path = ignored.to_string_lossy();
    let explicit = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            &ignored_path,
        ],
        b"",
        config.path(),
    )?;
    let explicit_value: serde_json::Value = serde_json::from_slice(&explicit.stdout)?;
    assert_eq!(
        explicit_value
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn discovery_does_not_follow_symlinked_directories() -> Result<(), TestError> {
    use std::os::unix::fs::symlink;

    let directory = tempdir()?;
    let outside = tempdir()?;
    let config = tempdir()?;
    fs::write(outside.path().join("linked.txt"), b"linked\r\n")?;
    symlink(outside.path(), directory.path().join("linked"))?;
    let path = directory.path().to_string_lossy();
    let output = run(
        &[
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
            &path,
        ],
        b"",
        config.path(),
    )?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    assert_eq!(
        value
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_closed_output_pipe_is_success() -> Result<(), TestError> {
    let directory = tempdir()?;
    let config = tempdir()?;
    let file = directory.path().join("many-findings.txt");
    fs::write(&file, "ｱ".repeat(5_000))?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_aozora-proof"))
        .args([
            "check",
            "--orthography",
            "mixed",
            "--no-input",
            "--format",
            "json",
        ])
        .arg(file)
        .env("XDG_CONFIG_HOME", config.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    drop(child.stdout.take());

    assert!(child.wait()?.success());
    Ok(())
}

#[test]
fn unknown_configuration_key_has_a_suggestion() -> Result<(), TestError> {
    let directory = tempdir()?;
    let config_home = tempdir()?;
    fs::write(
        directory.path().join(".aozora-proof.toml"),
        "orthograpy = \"mixed\"\n",
    )?;
    fs::write(directory.path().join("work.txt"), b"work\r\n")?;
    let path = directory.path().to_string_lossy();
    let output = run(&["check", "--no-input", &path], b"", config_home.path())?;
    let stderr = String::from_utf8(output.stderr)?;

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr.contains("did you mean \"orthography\"?"));
    Ok(())
}

#[test]
fn configuration_precedence_is_flag_environment_project_user() -> Result<(), TestError> {
    let directory = tempdir()?;
    let config_home = tempdir()?;
    let user_directory = config_home.path().join("aozora-proof");
    fs::create_dir_all(&user_directory)?;
    fs::write(
        user_directory.join("config.toml"),
        "orthography = \"modern\"\n",
    )?;
    fs::write(
        directory.path().join(".aozora-proof.toml"),
        "orthography = \"traditional\"\n",
    )?;
    let file = directory.path().join("work.txt");
    fs::write(&file, "來\r\n".as_bytes())?;
    let path = file.to_string_lossy();

    let project = run(&["config", "show", &path], b"", config_home.path())?;
    let project_text = String::from_utf8(project.stdout)?;
    assert!(project_text.contains("orthography = traditional"));

    let environment = run_with_environment(
        &["config", "show", &path],
        b"",
        config_home.path(),
        &[("AOZORA_PROOF_ORTHOGRAPHY", "mixed")],
    )?;
    let environment_text = String::from_utf8(environment.stdout)?;
    assert!(environment_text.contains("orthography = mixed"));
    assert!(environment_text.contains("AOZORA_PROOF_ORTHOGRAPHY"));

    let flag = run_with_environment(
        &[
            "check",
            "--orthography",
            "modern",
            "--no-input",
            "--format",
            "json",
            &path,
        ],
        b"",
        config_home.path(),
        &[("AOZORA_PROOF_ORTHOGRAPHY", "mixed")],
    )?;
    assert!(
        String::from_utf8(flag.stdout)?.contains("aozora::proof::orthography::modern_candidate")
    );
    Ok(())
}

#[test]
fn review_rejects_a_non_terminal() -> Result<(), TestError> {
    let directory = tempdir()?;
    let config = tempdir()?;
    let file = directory.path().join("work.txt");
    fs::write(&file, b"work\r\n")?;
    let path = file.to_string_lossy();
    let output = run(
        &["review", "--orthography", "mixed", "--no-input", &path],
        b"",
        config.path(),
    )?;

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)?.contains("interactive terminal"));
    Ok(())
}

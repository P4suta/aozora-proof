use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Resolved;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Input {
    pub(crate) path: Option<PathBuf>,
    pub(crate) label: String,
}

impl Input {
    pub(crate) const fn is_stdin(&self) -> bool {
        self.path.is_none()
    }
}

#[derive(Debug)]
pub(crate) struct DiscoveryError {
    message: String,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for DiscoveryError {}

pub(crate) fn discover(
    requested: &[PathBuf],
    config: &Resolved,
) -> Result<Vec<Input>, DiscoveryError> {
    let requested = if requested.is_empty() {
        vec![PathBuf::from("-")]
    } else {
        requested.to_vec()
    };
    let current = env::current_dir().map_err(|source| message(source.to_string()))?;
    let mut inputs = Vec::new();
    let mut stdin_seen = false;

    for path in &requested {
        if path.as_os_str() == "-" {
            if stdin_seen {
                return Err(message("standard input may be specified only once"));
            }
            stdin_seen = true;
            inputs.push(Input {
                path: None,
                label: "<stdin>".to_owned(),
            });
            continue;
        }
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            current.join(path)
        };
        let metadata = fs::metadata(&absolute)
            .map_err(|source| message(format!("{}: {source}", path.display())))?;
        if metadata.is_file() {
            inputs.push(Input {
                path: Some(absolute),
                label: normalize_label(path, &current),
            });
        } else if metadata.is_dir() {
            walk_directory(&absolute, &current, config, &mut inputs)?;
        } else {
            return Err(message(format!(
                "{} is neither a file nor a directory",
                path.display()
            )));
        }
    }

    inputs.sort_by(|left, right| left.label.cmp(&right.label));
    let mut labels = BTreeSet::new();
    inputs.retain(|input| labels.insert(input.label.clone()));
    Ok(inputs)
}

fn walk_directory(
    root: &Path,
    current: &Path,
    config: &Resolved,
    inputs: &mut Vec<Input>,
) -> Result<(), DiscoveryError> {
    let ignores = if config.respect_ignore {
        IgnoreRules::load(root)?
    } else {
        IgnoreRules::default()
    };
    Walker {
        root,
        current,
        config,
        ignores: &ignores,
        inputs,
    }
    .walk(root)
}

struct Walker<'a> {
    root: &'a Path,
    current: &'a Path,
    config: &'a Resolved,
    ignores: &'a IgnoreRules,
    inputs: &'a mut Vec<Input>,
}

impl Walker<'_> {
    fn walk(&mut self, directory: &Path) -> Result<(), DiscoveryError> {
        let mut entries: Vec<fs::DirEntry> = fs::read_dir(directory)
            .map_err(|source| message(source.to_string()))?
            .collect::<Result<_, _>>()
            .map_err(|source| message(source.to_string()))?;
        entries.sort_by_key(fs::DirEntry::file_name);

        for entry in entries {
            self.visit(&entry)?;
        }
        Ok(())
    }

    fn visit(&mut self, entry: &fs::DirEntry) -> Result<(), DiscoveryError> {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            return Ok(());
        }
        let relative = path.strip_prefix(self.root).unwrap_or(&path);
        let relative_label = normalize(relative);
        let file_type = entry
            .file_type()
            .map_err(|source| message(source.to_string()))?;
        if self.ignores.ignored(&relative_label, file_type.is_dir()) {
            return Ok(());
        }
        if file_type.is_dir() {
            if !file_type.is_symlink() {
                self.walk(&path)?;
            }
            return Ok(());
        }
        if file_type.is_symlink() && fs::metadata(&path).is_ok_and(|meta| meta.is_dir()) {
            return Ok(());
        }
        if path.extension().and_then(|value| value.to_str()) != Some("txt")
            || !self.included(&relative_label)
        {
            return Ok(());
        }
        self.inputs.push(Input {
            label: normalize_label(&path, self.current),
            path: Some(path),
        });
        Ok(())
    }

    fn included(&self, relative: &str) -> bool {
        (self.config.include.is_empty()
            || self
                .config
                .include
                .iter()
                .any(|pattern| wildcard_match(pattern, relative)))
            && !self
                .config
                .exclude
                .iter()
                .any(|pattern| wildcard_match(pattern, relative))
    }
}

#[derive(Debug, Default)]
struct IgnoreRules {
    patterns: Vec<IgnorePattern>,
}

impl IgnoreRules {
    fn load(root: &Path) -> Result<Self, DiscoveryError> {
        let mut patterns = Vec::new();
        for name in [".gitignore", ".ignore", ".aozora-proofignore"] {
            let path = root.join(name);
            if !path.is_file() {
                continue;
            }
            let content = fs::read_to_string(&path)
                .map_err(|source| message(format!("{}: {source}", path.display())))?;
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (negated, value) = line
                    .strip_prefix('!')
                    .map_or((false, line), |value| (true, value));
                patterns.push(IgnorePattern {
                    negated,
                    directory_only: value.ends_with('/'),
                    pattern: value.trim_matches('/').to_owned(),
                });
            }
        }
        Ok(Self { patterns })
    }

    fn ignored(&self, path: &str, is_directory: bool) -> bool {
        let mut ignored = false;
        for pattern in &self.patterns {
            if pattern.directory_only && !is_directory {
                continue;
            }
            if ignore_match(&pattern.pattern, path) {
                ignored = !pattern.negated;
            }
        }
        ignored
    }
}

#[derive(Debug)]
struct IgnorePattern {
    pattern: String,
    negated: bool,
    directory_only: bool,
}

fn ignore_match(pattern: &str, path: &str) -> bool {
    if pattern.contains('/') {
        wildcard_match(pattern, path)
    } else {
        path.split('/')
            .any(|component| wildcard_match(pattern, component))
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

fn normalize_label(path: &Path, current: &Path) -> String {
    normalize(path.strip_prefix(current).unwrap_or(path))
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn message(value: impl Into<String>) -> DiscoveryError {
    DiscoveryError {
        message: value.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_and_ignore_matching_cover_common_forms() {
        assert!(wildcard_match("**/*.txt", "work/chapter.txt"));
        assert!(ignore_match("target", "target/cache.txt"));
        assert!(!wildcard_match("*.md", "work.txt"));
    }
}

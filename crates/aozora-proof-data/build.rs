//! Generate the character-classification lookup tables from vendored sources.

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

type LevelMap = BTreeMap<u32, u8>;
type MenKuTenMap = BTreeMap<u32, (u8, u8, u8)>;

#[derive(Debug, thiserror::Error)]
enum BuildError {
    #[error("required Cargo environment variable {name} is unavailable")]
    Environment {
        name: &'static str,
        #[source]
        source: env::VarError,
    },
    #[error("{}: could not {operation}", path.display())]
    Io {
        path: PathBuf,
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid hexadecimal value {value:?} in source line {line}")]
    Hex {
        line: usize,
        value: String,
        #[source]
        source: std::num::ParseIntError,
    },
    #[error("invalid JIS cell in source line {line}")]
    Cell { line: usize },
    #[error("could not format generated Rust")]
    Format {
        #[source]
        source: fmt::Error,
    },
}

fn upsert(map: &mut BTreeMap<u32, u8>, codepoint: u32, level: u8) {
    map.entry(codepoint)
        .and_modify(|existing| {
            if level < *existing {
                *existing = level;
            }
        })
        .or_insert(level);
}

fn parse_jis(text: &str) -> Result<(LevelMap, MenKuTenMap), BuildError> {
    let mut levels = BTreeMap::new();
    let mut men_ku_ten = BTreeMap::new();
    for (line_index, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with("##") {
            continue;
        }
        let line_number = line_index.saturating_add(1);
        let mut fields = line.split('\t');
        let Some(cell) = fields.next() else {
            continue;
        };
        let Some(ucs) = fields.next() else {
            continue;
        };

        let [plane, b'-', _, _, _, _] = *cell.as_bytes() else {
            continue;
        };
        if plane != b'3' && plane != b'4' {
            continue;
        }
        let Some(rrcc_hex) = cell.get(2..6) else {
            continue;
        };
        let rrcc = u16::from_str_radix(rrcc_hex, 16).map_err(|source| BuildError::Hex {
            line: line_number,
            value: rrcc_hex.to_owned(),
            source,
        })?;
        let [row, column] = rrcc.to_be_bytes();
        let ku = row
            .checked_sub(0x20)
            .ok_or(BuildError::Cell { line: line_number })?;
        let ten = column
            .checked_sub(0x20)
            .ok_or(BuildError::Cell { line: line_number })?;

        let Some(hex) = ucs.strip_prefix("U+") else {
            continue;
        };
        if hex.is_empty() || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let codepoint = u32::from_str_radix(hex, 16).map_err(|source| BuildError::Hex {
            line: line_number,
            value: hex.to_owned(),
            source,
        })?;

        let level = if plane == b'4' {
            4
        } else if line.contains("[2000]") || line.contains("[2004]") {
            3
        } else if ku <= 47 {
            1
        } else {
            2
        };
        upsert(&mut levels, codepoint, level);

        let men = if plane == b'4' { 2 } else { 1 };
        men_ku_ten.entry(codepoint).or_insert((men, ku, ten));

        for marker in ["Fullwidth: U+", "Windows: U+"] {
            if let Some(rest) = line.split(marker).nth(1) {
                let alias: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
                if !alias.is_empty() {
                    let alias_codepoint =
                        u32::from_str_radix(&alias, 16).map_err(|source| BuildError::Hex {
                            line: line_number,
                            value: alias,
                            source,
                        })?;
                    upsert(&mut levels, alias_codepoint, level);
                }
            }
        }
    }
    Ok((levels, men_ku_ten))
}

fn parse_kyuji(text: &str) -> BTreeMap<u32, u32> {
    let mut kyuji = BTreeMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut columns = line.split('\t');
        let old = columns.next().and_then(|value| value.chars().next());
        let new = columns.next().and_then(|value| value.chars().next());
        if let (Some(old), Some(new)) = (old, new) {
            kyuji.insert(u32::from(old), u32::from(new));
        }
    }
    kyuji
}

fn main() -> Result<(), BuildError> {
    let manifest = required_env("CARGO_MANIFEST_DIR")?;
    let data = Path::new(&manifest).join("data");
    println!("cargo::rerun-if-changed=data/jisx0213-2004-std.txt");
    println!("cargo::rerun-if-changed=data/joyo-kyujitai.tsv");

    let jis_path = data.join("jisx0213-2004-std.txt");
    let kyuji_path = data.join("joyo-kyujitai.tsv");
    let jis_text = read_source(&jis_path)?;
    let kyuji_text = read_source(&kyuji_path)?;
    let (levels, men_ku_ten) = parse_jis(&jis_text)?;
    let kyuji = parse_kyuji(&kyuji_text);

    let capacity = levels
        .len()
        .saturating_mul(16)
        .saturating_add(men_ku_ten.len().saturating_mul(20))
        .saturating_add(256);
    let mut output = String::with_capacity(capacity);
    output.push_str("// @generated by build.rs — do not edit.\n");
    output.push_str("static JIS_LEVELS: &[(u32, u8)] = &[\n");
    for (codepoint, level) in &levels {
        writeln!(output, "    ({codepoint:#06x}, {level}),")
            .map_err(|source| BuildError::Format { source })?;
    }
    output.push_str("];\nstatic GAIJI_MENKUTEN: &[(u32, u8, u8, u8)] = &[\n");
    for (codepoint, (men, ku, ten)) in &men_ku_ten {
        writeln!(output, "    ({codepoint:#06x}, {men}, {ku}, {ten}),")
            .map_err(|source| BuildError::Format { source })?;
    }
    output.push_str("];\nstatic KYUJI_TO_SHINJI: &[(u32, char)] = &[\n");
    for (old, new) in &kyuji {
        writeln!(output, "    ({old:#06x}, '\\u{{{new:04x}}}'),")
            .map_err(|source| BuildError::Format { source })?;
    }
    output.push_str("];\n");

    let out_dir = required_env("OUT_DIR")?;
    let output_path = Path::new(&out_dir).join("jis_tables.rs");
    fs::write(&output_path, output).map_err(|source| BuildError::Io {
        path: output_path,
        operation: "write generated lookup tables",
        source,
    })
}

fn required_env(name: &'static str) -> Result<String, BuildError> {
    env::var(name).map_err(|source| BuildError::Environment { name, source })
}

fn read_source(path: &Path) -> Result<String, BuildError> {
    fs::read_to_string(path).map_err(|source| BuildError::Io {
        path: path.to_path_buf(),
        operation: "read a table source",
        source,
    })
}

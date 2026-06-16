//! `aozora-proof-data` — character-classification tables for the proofreading
//! engine, baked at build time from the Project X0213 mapping table.
//!
//! The core conformance question — *"may this character appear literally in
//! conformant 青空文庫 text, or must it be written as a 外字注記?"* — is
//! [`jis_level`] plus [`Suijun::is_jisx0208`]. [`is_platform_dependent`]
//! isolates the especially-non-portable 機種依存文字 band.

#![forbid(unsafe_code)]

use std::sync::OnceLock;

// Generated tables, sorted by codepoint:
//   static JIS_LEVELS: &[(u32, u8)]
//   static KYUJI_TO_SHINJI: &[(u32, char)]
//   static GAIJI_MENKUTEN: &[(u32, u8, u8, u8)]
include!(concat!(env!("OUT_DIR"), "/jis_tables.rs"));

/// JIS 水準 (level) classification of a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suijun {
    /// 第1水準 — JIS X 0208 (common kanji + symbols).
    Level1,
    /// 第2水準 — JIS X 0208 (less-common kanji).
    Level2,
    /// 第3水準 — JIS X 0213 plane 1 additions (need 外字注記).
    Level3,
    /// 第4水準 — JIS X 0213 plane 2 (need 外字注記).
    Level4,
    /// Outside JIS X 0213 entirely.
    Outside,
}

impl Suijun {
    /// Whether the character is in JIS X 0208 (第1/第2水準) — i.e. usable
    /// literally in conformant Aozora text without a 外字注記.
    #[must_use]
    pub const fn is_jisx0208(self) -> bool {
        matches!(self, Self::Level1 | Self::Level2)
    }

    /// Whether the character is in JIS X 0213 but beyond JIS X 0208
    /// (第3/第4水準) — representable only via a 外字注記.
    #[must_use]
    pub const fn is_jisx0213_only(self) -> bool {
        matches!(self, Self::Level3 | Self::Level4)
    }

    /// Japanese label for the 水準 (`第1水準` … `第4水準` / `JIS X 0213外`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Level1 => "第1水準",
            Self::Level2 => "第2水準",
            Self::Level3 => "第3水準",
            Self::Level4 => "第4水準",
            Self::Outside => "JIS X 0213外",
        }
    }
}

/// Classify a character's JIS 水準.
#[must_use]
pub fn jis_level(c: char) -> Suijun {
    let cp = u32::from(c);
    JIS_LEVELS
        .binary_search_by_key(&cp, |&(k, _)| k)
        .map_or(Suijun::Outside, |i| match JIS_LEVELS[i].1 {
            1 => Suijun::Level1,
            2 => Suijun::Level2,
            3 => Suijun::Level3,
            _ => Suijun::Level4,
        })
}

/// Whether `c` is a 機種依存文字 (platform-dependent character): encodable in
/// CP932 (Windows-31J — `encoding_rs`'s `Shift_JIS`) but outside JIS X 0208.
///
/// Such characters (NEC special characters, NEC-selected IBM extensions, IBM
/// extensions — e.g. `①`, `㈱`, `㍻`) appear representable on some platforms
/// but are non-portable, so conformant Aozora text must spell them as 外字注記.
///
/// ASCII is never platform-dependent (half/full-width handling is a separate
/// concern), so it is excluded up front.
#[must_use]
pub fn is_platform_dependent(c: char) -> bool {
    if c.is_ascii() || jis_level(c).is_jisx0208() {
        return false;
    }
    let mut buf = [0u8; 4];
    let encoded = c.encode_utf8(&mut buf);
    let (_, _, had_errors) = encoding_rs::SHIFT_JIS.encode(encoded);
    !had_errors
}

/// The 新字体 (modern form) for a 旧字体 / 異体字 character, if one is recorded
/// in the 常用漢字表-derived table. Returns `None` for characters that are
/// already a standard form or have no recorded counterpart.
#[must_use]
pub fn shinji_for(c: char) -> Option<char> {
    KYUJI_TO_SHINJI
        .binary_search_by_key(&u32::from(c), |&(k, _)| k)
        .ok()
        .map(|i| KYUJI_TO_SHINJI[i].1)
}

/// A character's JIS X 0213 面区点 (plane-row-cell) position plus its 水準.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenKuTen {
    /// Plane (面): 1 or 2.
    pub men: u8,
    /// Row (区).
    pub ku: u8,
    /// Cell (点).
    pub ten: u8,
    /// The 水準 the cell belongs to.
    pub level: Suijun,
}

/// The JIS X 0213 面区点 position (and 水準) of `c`, if it is a JIS X 0213
/// character. The 外字注記 form is written `第N水準 men-ku-ten`.
#[must_use]
pub fn men_ku_ten(c: char) -> Option<MenKuTen> {
    let cp = u32::from(c);
    let i = GAIJI_MENKUTEN
        .binary_search_by_key(&cp, |&(k, ..)| k)
        .ok()?;
    let (_, men, ku, ten) = GAIJI_MENKUTEN[i];
    Some(MenKuTen {
        men,
        ku,
        ten,
        level: jis_level(c),
    })
}

/// The character at JIS X 0213 面区点 `men`-`ku`-`ten`, if the cell is assigned.
#[must_use]
pub fn char_at_men_ku_ten(men: u8, ku: u8, ten: u8) -> Option<char> {
    GAIJI_MENKUTEN
        .iter()
        .find(|&&(_, m, k, t)| m == men && k == ku && t == ten)
        .and_then(|&(cp, ..)| char::from_u32(cp))
}

/// The vendored 外字注記辞書 (CC0), parsed once into (description, character).
fn gaiji_dict() -> &'static [(&'static str, char)] {
    static DICT: OnceLock<Vec<(&'static str, char)>> = OnceLock::new();
    DICT.get_or_init(|| {
        const RAW: &str = include_str!("../data/aozora-gaiji-chuki.tsv");
        RAW.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| {
                let (desc, hex) = line.split_once('\t')?;
                let cp = u32::from_str_radix(hex.trim(), 16).ok()?;
                char::from_u32(cp).map(|c| (desc, c))
            })
            .collect()
    })
}

/// The 外字注記 descriptions recorded for `c` (e.g. `弓＋椀のつくり`).
#[must_use]
pub fn gaiji_descriptions(c: char) -> Vec<&'static str> {
    gaiji_dict()
        .iter()
        .filter(|&&(_, ch)| ch == c)
        .map(|&(desc, _)| desc)
        .collect()
}

/// Search the 外字注記辞書 for descriptions containing `query`, returning
/// (description, character) pairs. An empty query returns nothing.
#[must_use]
pub fn gaiji_search(query: &str) -> Vec<(&'static str, char)> {
    if query.is_empty() {
        return Vec::new();
    }
    gaiji_dict()
        .iter()
        .filter(|&&(desc, _)| desc.contains(query))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_levels() {
        assert_eq!(jis_level('\u{4E9C}'), Suijun::Level1); // 亜  — 3-3021, untagged
        assert_eq!(jis_level('\u{4FF1}'), Suijun::Level3); // 俱  — 3-2E21 [2004]
        assert_eq!(jis_level('\u{20089}'), Suijun::Level4); // 𠂉系 — 4-2121
        assert_eq!(jis_level('\u{1F363}'), Suijun::Outside); // 🍣 — not in JIS X 0213
        assert_eq!(jis_level('\u{FF5C}'), Suijun::Level1); // ｜ ruby marker, via Fullwidth alias
        assert_eq!(jis_level('\u{FF03}'), Suijun::Level1); // ＃ annotation marker
    }

    #[test]
    fn old_to_new_forms() {
        assert_eq!(shinji_for('\u{4F5B}'), Some('\u{4ECF}')); // 佛 → 仏
        assert_eq!(shinji_for('\u{4F86}'), Some('\u{6765}')); // 來 → 来
        assert_eq!(shinji_for('\u{4ECF}'), None); // 仏 is already 新字体
        assert_eq!(shinji_for('あ'), None);
    }

    #[test]
    fn gaiji_men_ku_ten_roundtrip() {
        // 俱 (U+4FF1) is at 3-2E21 → men 1, ku 14, ten 1, 第3水準.
        let m = men_ku_ten('\u{4FF1}').expect("俱 is in JIS X 0213");
        assert_eq!((m.men, m.ku, m.ten), (1, 14, 1));
        assert_eq!(m.level, Suijun::Level3);
        assert_eq!(char_at_men_ku_ten(1, 14, 1), Some('\u{4FF1}'));
        assert_eq!(char_at_men_ku_ten(9, 9, 9), None);
    }

    #[test]
    fn gaiji_dict_lookup_and_search() {
        assert!(gaiji_search("").is_empty());
        let descs = gaiji_descriptions('\u{20089}'); // has a 外字注記 description
        assert!(!descs.is_empty());
        // Searching for that description finds the same character back.
        assert!(
            gaiji_search(descs[0])
                .iter()
                .any(|&(_, c)| c == '\u{20089}')
        );
    }

    #[test]
    fn jisx0208_predicate() {
        assert!(jis_level('\u{4E9C}').is_jisx0208()); // 亜
        assert!(!jis_level('\u{4FF1}').is_jisx0208()); // 俱 (第3水準)
        assert!(jis_level('\u{4FF1}').is_jisx0213_only());
    }

    #[test]
    fn platform_dependent() {
        assert!(is_platform_dependent('\u{2460}')); // ① NEC special, CP932 ∖ 0208
        assert!(!is_platform_dependent('\u{4E9C}')); // 亜 in 0208
        assert!(!is_platform_dependent('A')); // ASCII guard
        assert!(!is_platform_dependent('\u{1F363}')); // 🍣 not in CP932
    }

    #[test]
    fn table_sanity_counts() {
        let jisx0208 = JIS_LEVELS
            .iter()
            .filter(|&&(_, l)| l == 1 || l == 2)
            .count();
        let level4 = JIS_LEVELS.iter().filter(|&&(_, l)| l == 4).count();
        // Plane-1 not [2000]/[2004] = 6918 cells, plus the ~90 full-width
        // aliases (｜＃［］ and full-width punctuation), minus the few cells
        // that share a single Unicode scalar.
        assert!(
            (6900..=7050).contains(&jisx0208),
            "JIS X 0208 distinct count {jisx0208} out of range"
        );
        // Plane 2 = 第4水準 = 2436 cells, injective to Unicode.
        assert!(
            (2400..=2436).contains(&level4),
            "第4水準 distinct count {level4} out of range"
        );
    }
}

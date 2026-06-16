//! `aozora-proof-data` — character-classification tables for the proofreading
//! engine, baked at build time from the Project X0213 mapping table.
//!
//! The core conformance question — *"may this character appear literally in
//! conformant 青空文庫 text, or must it be written as a 外字注記?"* — is
//! [`jis_level`] plus [`Suijun::is_jisx0208`]. [`is_platform_dependent`]
//! isolates the especially-non-portable 機種依存文字 band.

#![forbid(unsafe_code)]

// `static JIS_LEVELS: &[(u32, u8)]`, sorted by codepoint.
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

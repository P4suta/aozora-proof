//! Gaiji (外字) lookup and search.
//!
//! Bidirectional access over the JIS X 0213 面区点 table and the CC0 外字注記
//! dictionary: character ⇔ 面区点 ⇔ Unicode, plus description search and a
//! suggested 外字注記 form. Backed by `aozora-proof-data`.

use aozora_proof_data::{
    MenKuTen, Suijun, char_at_men_ku_ten, gaiji_descriptions, gaiji_search, men_ku_ten,
};

/// Everything known about a character that is relevant to 外字注記.
#[derive(Debug, Clone)]
pub struct GaijiInfo {
    /// The character itself.
    pub character: char,
    /// Its Unicode scalar value.
    pub codepoint: u32,
    /// Its JIS X 0213 面区点 position + 水準, if it is a JIS X 0213 character.
    pub men_ku_ten: Option<MenKuTen>,
    /// 外字注記 descriptions recorded for it (may be empty).
    pub descriptions: Vec<&'static str>,
    /// A suggested 外字注記 string, when the character needs one (第3/第4水準).
    pub chuki: Option<String>,
}

/// Look up everything known about `c`.
#[must_use]
pub fn lookup(c: char) -> GaijiInfo {
    let mkt = men_ku_ten(c);
    let descriptions = gaiji_descriptions(c);
    let chuki = mkt.and_then(|m| chuki_form(m, descriptions.first().copied()));
    GaijiInfo {
        character: c,
        codepoint: u32::from(c),
        men_ku_ten: mkt,
        descriptions,
        chuki,
    }
}

/// The character at a JIS X 0213 面区点 position, if the cell is assigned.
#[must_use]
pub fn from_men_ku_ten(men: u8, ku: u8, ten: u8) -> Option<char> {
    char_at_men_ku_ten(men, ku, ten)
}

/// Search 外字注記 descriptions for `query` (substring), returning
/// (description, character) pairs.
#[must_use]
pub fn search(query: &str) -> Vec<(&'static str, char)> {
    gaiji_search(query)
}

/// Build the 外字注記 form for a 第3/第4水準 cell:
/// `※［＃「desc」、第N水準 men-ku-ten］`. Returns `None` for 第1/第2水準
/// characters (which are usable literally and need no 注記).
fn chuki_form(m: MenKuTen, desc: Option<&str>) -> Option<String> {
    let level = match m.level {
        Suijun::Level3 => 3,
        Suijun::Level4 => 4,
        _ => return None,
    };
    let desc = desc.unwrap_or("");
    Some(format!(
        "※［＃「{desc}」、第{level}水準{}-{}-{}］",
        m.men, m.ku, m.ten
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_third_level_char() {
        let info = lookup('\u{4FF1}'); // 俱, 第3水準 1-14-1
        let m = info.men_ku_ten.expect("俱 is in JIS X 0213");
        assert_eq!((m.men, m.ku, m.ten), (1, 14, 1));
        assert!(info.chuki.as_deref().unwrap().contains("第3水準1-14-1"));
    }

    #[test]
    fn jisx0208_char_has_no_chuki() {
        let info = lookup('亜'); // 第1水準 — usable literally
        assert!(info.chuki.is_none());
    }

    #[test]
    fn men_ku_ten_roundtrips_and_search_works() {
        assert_eq!(from_men_ku_ten(1, 14, 1), Some('\u{4FF1}'));
        let descs = gaiji_descriptions('\u{20089}');
        assert!(!descs.is_empty());
        assert!(search(descs[0]).iter().any(|&(_, c)| c == '\u{20089}'));
    }
}

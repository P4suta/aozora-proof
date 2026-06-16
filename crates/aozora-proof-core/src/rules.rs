//! Rule documentation — the *why* behind each finding `code`.
//!
//! The check modules emit stable, namespaced `code`s (e.g.
//! `aozora::char::platform_dependent`). A `code` plus a one-line message tells
//! an editor *what* tripped; this module attaches the rationale and a good/bad
//! example so the CLI's `explain` subcommand can teach the rule rather than
//! merely flag it.
//!
//! Only the character-level codes this crate owns are documented here.
//! Notation findings reuse the upstream parser's `aozora::lex::*` codes, which
//! the parser documents.

/// Extended, human-facing documentation for one finding [`code`](Self::code).
///
/// Every field is a `'static str`, so the whole table is baked into the binary
/// and [`RuleDoc`] is cheap to copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleDoc {
    /// The stable code this documents (e.g. `aozora::char::platform_dependent`).
    pub code: &'static str,
    /// One-line title.
    pub title: &'static str,
    /// Why the rule exists / why the flagged text is a problem.
    pub rationale: &'static str,
    /// A short non-conformant example (what triggers the finding).
    pub example_bad: &'static str,
    /// The conformant alternative.
    pub example_good: &'static str,
    /// How to resolve the finding.
    pub fix: &'static str,
}

/// The documentation table, one entry per character-level code we emit.
///
/// Kept in step with the `codes` modules in [`crate::moji`],
/// [`crate::moji::file_checks`] and [`crate::kyuji`]; the
/// `every_owned_code_has_a_rule` test fails if a code ships without docs.
const RULES: &[RuleDoc] = &[
    RuleDoc {
        code: "aozora::char::utf8_bom",
        title: "先頭の UTF-8 BOM",
        rationale: "青空文庫テキストは BOM を含めない慣習です。BOM があると先頭行の処理や\
                    ファイル結合時に不整合を生むことがあります。",
        example_bad: "\u{feff}底本：……",
        example_good: "底本：……",
        fix: "ファイル先頭の BOM（バイト列 EF BB BF）を取り除いて保存し直します。",
    },
    RuleDoc {
        code: "aozora::char::crlf_expected",
        title: "改行コードが CR+LF ではない",
        rationale: "青空文庫の投稿規約は改行を CR+LF に揃えます。LF や CR のみだと\
                    配布環境によって体裁が崩れることがあります。",
        example_bad: "一行目<LF>二行目",
        example_good: "一行目<CR><LF>二行目",
        fix: "改行コードを CR+LF に統一して保存します。",
    },
    RuleDoc {
        code: "aozora::char::invalid_encoding",
        title: "文字コードを判定できない",
        rationale: "入力が UTF-8 でも Shift_JIS でもデコードできませんでした。青空文庫\
                    テキストはこのいずれかで保存します。",
        example_bad: "EUC-JP や壊れたバイト列",
        example_good: "UTF-8 または Shift_JIS で保存されたテキスト",
        fix: "UTF-8 か Shift_JIS で保存し直します。",
    },
    RuleDoc {
        code: "aozora::char::halfwidth_katakana",
        title: "半角カタカナ",
        rationale: "半角カタカナ（JIS X 0201）は本文に直接用いません。全角で表記します。",
        example_bad: "ｱｵｿﾞﾗ",
        example_good: "アオゾラ",
        fix: "対応する全角カタカナに置き換えます。",
    },
    RuleDoc {
        code: "aozora::char::platform_dependent",
        title: "機種依存文字",
        rationale: "CP932 にはあるが JIS X 0208 の外にある文字（①㈱℡ など）は環境依存で\
                    文字化けの恐れがあるため、本文に直接置かず外字注記にします。",
        example_bad: "第①巻",
        example_good: "第一巻（または外字注記で表記）",
        fix: "通常の文字に直すか、外字注記（［＃…］）に置き換えます。",
    },
    RuleDoc {
        code: "aozora::char::needs_gaiji_chuki",
        title: "外字注記が必要（JIS X 0213 第3・第4水準）",
        rationale: "JIS X 0208 の外（第3・第4水準）の文字は環境差が大きいため、本文に\
                    直接置かず外字注記で示します。",
        example_bad: "第3水準の漢字をそのまま本文に置く",
        example_good: "外字注記（［＃…］）で表記する",
        fix: "外字注記に置き換えます。`aozora-proof gaiji <文字>` で面区点を確認できます。",
    },
    RuleDoc {
        code: "aozora::char::not_in_jisx0213",
        title: "JIS X 0213 の範囲外",
        rationale: "JIS X 0213 に含まれない文字は、対応する通常字への置換か外字注記が\
                    必要です。",
        example_bad: "JIS X 0213 外の記号・漢字を本文に置く",
        example_good: "代替できる字に置換、または外字注記で表記",
        fix: "代替字に置き換えるか、外字注記で示します。",
    },
    RuleDoc {
        code: "aozora::kyuji::has_shinji_form",
        title: "新字体のある旧字体",
        rationale: "常用漢字表で新字体が定められている旧字体です。底本の方針に合わせて\
                    統一するか確認します。",
        example_bad: "廣島",
        example_good: "広島",
        fix: "底本の方針に従い、必要なら新字体へ統一します（`aozora-proof check --fix`）。",
    },
];

/// Look up the extended documentation for a finding `code`.
///
/// Returns `None` for codes we do not own (notation `aozora::lex::*` codes,
/// or an unknown string).
#[must_use]
pub fn explain(code: &str) -> Option<RuleDoc> {
    RULES.iter().copied().find(|r| r.code == code)
}

/// Every documented rule, in table order — for listing in `--help`-style output.
#[must_use]
pub const fn all_rules() -> &'static [RuleDoc] {
    RULES
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every character-level code the crate can emit must have a `RuleDoc`.
    /// Update this list (and `RULES`) when adding a new check.
    #[test]
    fn every_owned_code_has_a_rule() {
        let owned = [
            "aozora::char::utf8_bom",
            "aozora::char::crlf_expected",
            "aozora::char::invalid_encoding",
            "aozora::char::halfwidth_katakana",
            "aozora::char::platform_dependent",
            "aozora::char::needs_gaiji_chuki",
            "aozora::char::not_in_jisx0213",
            "aozora::kyuji::has_shinji_form",
        ];
        for code in owned {
            assert!(explain(code).is_some(), "missing RuleDoc for {code}");
        }
    }

    #[test]
    fn rule_codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for r in all_rules() {
            assert!(seen.insert(r.code), "duplicate RuleDoc for {}", r.code);
        }
    }

    #[test]
    fn unknown_code_has_no_rule() {
        assert!(explain("aozora::char::does_not_exist").is_none());
        assert!(explain("aozora::lex::unterminated_ruby").is_none());
    }
}

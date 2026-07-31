//! Rule catalog and official-requirement coverage.

use crate::{DetectionClass, FixApplicability, RuleCategory, Severity};

const CHECKLIST: &str = "https://www.aozora.gr.jp/KOSAKU/textfile_checklist/";
const PROOFREADING: &str = "https://www.aozora.gr.jp/aozora-manual/index-proofreading.html";
const AOZORA_PARSER: &str = "https://docs.rs/aozora/latest/aozora/";

/// Stable proofreader-owned rule codes.
pub mod codes {
    /// Leading UTF-8 byte-order mark.
    pub const BOM: &str = "aozora::proof::encoding::bom";
    /// Non-CRLF or mixed line endings.
    pub const LINE_ENDING: &str = "aozora::proof::encoding::line_ending";
    /// Missing final line ending.
    pub const FINAL_NEWLINE: &str = "aozora::proof::encoding::final_newline";
    /// Source is not submission-format Shift_JIS.
    pub const SOURCE_ENCODING: &str = "aozora::proof::encoding::source_encoding";
    /// Source is undecodable.
    pub const INVALID_ENCODING: &str = "aozora::proof::encoding::invalid";
    /// JIS X 0201 half-width kana or punctuation.
    pub const HALFWIDTH_KANA: &str = "aozora::proof::character::halfwidth_kana";
    /// CP932 platform-dependent scalar.
    pub const PLATFORM_DEPENDENT: &str = "aozora::proof::character::platform_dependent";
    /// Character must be represented by an external-character annotation.
    pub const NEEDS_GAIJI: &str = "aozora::proof::character::needs_gaiji";
    /// Forbidden control scalar.
    pub const CONTROL: &str = "aozora::proof::character::control";
    /// Literal tab requiring layout review.
    pub const TAB: &str = "aozora::proof::layout::tab";
    /// Literal form feed requiring page-break review.
    pub const FORM_FEED: &str = "aozora::proof::layout::form_feed";
    /// ASCII ruby boundary marker with parser-confirmed context.
    pub const ASCII_RUBY_MARKER: &str = "aozora::proof::ruby::ascii_boundary_marker";
    /// Non-canonical voiced iteration mark.
    pub const ITERATION_MARK: &str = "aozora::proof::character::iteration_mark";
    /// Traditional form found under modern policy.
    pub const MODERN_CANDIDATE: &str = "aozora::proof::orthography::modern_candidate";
    /// Modern form found under traditional policy.
    pub const TRADITIONAL_CANDIDATE: &str = "aozora::proof::orthography::traditional_candidate";
    /// OCR-confusable character in a suspicious script context.
    pub const OCR_SIMILAR: &str = "aozora::proof::character::ocr_similar";
    /// Context-dependent ASCII spacing or punctuation.
    pub const SPACING: &str = "aozora::proof::layout::spacing";
    /// `ケ` or `ヶ` usage requiring base-edition confirmation.
    pub const SMALL_KE: &str = "aozora::proof::orthography::small_ke";
    /// Ruby appears over-grouped or under-grouped.
    pub const RUBY_GROUPING: &str = "aozora::proof::ruby::grouping";
    /// Opening symbol legend is absent or inconsistent.
    pub const OPENING_LEGEND: &str = "aozora::proof::bibliography::opening_legend";
    /// Closing bibliographical matter is absent or incomplete.
    pub const CLOSING_BIBLIOGRAPHY: &str = "aozora::proof::bibliography::closing_bibliography";
    /// Compare every scalar with the base edition.
    pub const MANUAL_BASE_TEXT: &str = "aozora::proof::manual::base_text_comparison";
    /// Resolve layout that depends on meaning or page image.
    pub const MANUAL_LAYOUT: &str = "aozora::proof::manual::semantic_layout";
    /// Resolve copyright and bibliographical judgement.
    pub const MANUAL_RIGHTS: &str = "aozora::proof::manual::rights_and_bibliography";
    /// Confirm the submission filename against the work and contributor record.
    pub const MANUAL_FILENAME: &str = "aozora::proof::manual::submission_filename";
    /// Remove resource forks or host-specific metadata.
    pub const MANUAL_RESOURCE_FORK: &str = "aozora::proof::manual::resource_fork";
}

/// Complete metadata for one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuleDoc {
    /// Stable identity.
    pub code: &'static str,
    /// Category.
    pub category: RuleCategory,
    /// Default severity.
    pub default_severity: Severity,
    /// Canonical English title.
    pub title: &'static str,
    /// Japanese title.
    pub title_ja: &'static str,
    /// Canonical English rationale.
    pub rationale: &'static str,
    /// Japanese rationale.
    pub rationale_ja: &'static str,
    /// Detection classification.
    pub detection: DetectionClass,
    /// Strongest correction the rule can offer.
    pub fix: Option<FixApplicability>,
    /// Normative or upstream authority.
    pub authority_url: &'static str,
    /// Triggering example.
    pub example_bad: &'static str,
    /// Desired example.
    pub example_good: &'static str,
}

macro_rules! rule {
    (
        $code:expr, $category:ident, $severity:ident, $title:expr, $title_ja:expr,
        $rationale:expr, $rationale_ja:expr, $detection:ident, $fix:expr,
        $authority:expr, $bad:expr, $good:expr
    ) => {
        RuleDoc {
            code: $code,
            category: RuleCategory::$category,
            default_severity: Severity::$severity,
            title: $title,
            title_ja: $title_ja,
            rationale: $rationale,
            rationale_ja: $rationale_ja,
            detection: DetectionClass::$detection,
            fix: $fix,
            authority_url: $authority,
            example_bad: $bad,
            example_good: $good,
        }
    };
}

const RULES: &[RuleDoc] = &[
    rule!(
        codes::BOM,
        Encoding,
        Warning,
        "Leading UTF-8 BOM",
        "先頭の UTF-8 BOM",
        "Submission text does not include a byte-order mark.",
        "提出用テキストには BOM を含めません。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "<BOM>底本：…",
        "底本：…"
    ),
    rule!(
        codes::LINE_ENDING,
        Encoding,
        Error,
        "Line endings are not CRLF",
        "改行コードが CRLF ではない",
        "Aozora submission files use the Windows CRLF convention.",
        "青空文庫の提出ファイルは Windows 形式の CRLF に統一します。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "first<LF>second",
        "first<CRLF>second"
    ),
    rule!(
        codes::FINAL_NEWLINE,
        Encoding,
        Warning,
        "Final newline is missing",
        "末尾の改行がない",
        "The final bibliographical line is followed by a line ending.",
        "書誌情報の最終行の後にも改行を置きます。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "…皆さんです。<EOF>",
        "…皆さんです。<CRLF><EOF>"
    ),
    rule!(
        codes::SOURCE_ENCODING,
        Encoding,
        Warning,
        "Source is not Shift_JIS",
        "提出形式が Shift_JIS ではない",
        "Submission text is encoded as Shift_JIS after every character is made representable.",
        "全文字の表現可能性を確認した上で Shift_JIS に変換します。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "UTF-8 source",
        "lossless Shift_JIS source"
    ),
    rule!(
        codes::INVALID_ENCODING,
        Encoding,
        Error,
        "Source cannot be decoded",
        "文字コードを判定できない",
        "The input must decode without loss as UTF-8 or Shift_JIS.",
        "入力は UTF-8 または Shift_JIS として無損失に読める必要があります。",
        Automatic,
        None,
        CHECKLIST,
        "invalid byte sequence",
        "valid UTF-8 or Shift_JIS"
    ),
    rule!(
        codes::HALFWIDTH_KANA,
        Character,
        Error,
        "Half-width kana",
        "半角カナ",
        "JIS X 0201 kana and punctuation are written using their unambiguous full-width equivalents.",
        "JIS X 0201 の半角カナと約物は確定した全角等価文字にします。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "ｱｵｿﾞﾗ",
        "アオゾラ"
    ),
    rule!(
        codes::PLATFORM_DEPENDENT,
        Character,
        Error,
        "Platform-dependent character",
        "機種依存文字",
        "CP932 extensions outside JIS X 0208 are not portable submission characters.",
        "JIS X 0208 外の CP932 拡張文字は提出文字として移植性がありません。",
        Automatic,
        Some(FixApplicability::Review),
        CHECKLIST,
        "①",
        "一 or an external-character annotation"
    ),
    rule!(
        codes::NEEDS_GAIJI,
        Character,
        Warning,
        "External-character annotation required",
        "外字注記が必要",
        "A scalar outside JIS X 0208 is represented using the prescribed annotation.",
        "JIS X 0208 外の文字は規定の外字注記で表します。",
        Automatic,
        Some(FixApplicability::Review),
        CHECKLIST,
        "俱",
        "※［＃…、第3水準1-14-1］"
    ),
    rule!(
        codes::CONTROL,
        Character,
        Error,
        "Forbidden control character",
        "使用できない制御文字",
        "Invisible control characters cannot carry stable submission meaning.",
        "不可視制御文字は提出本文で安定した意味を持ちません。",
        Automatic,
        None,
        CHECKLIST,
        "<NUL>",
        "no control character"
    ),
    rule!(
        codes::TAB,
        Layout,
        Warning,
        "Tab requires layout review",
        "タブの配置確認",
        "Tab width depends on the viewer, so the base layout must be represented explicitly.",
        "タブ幅は環境依存のため、底本の配置を注記などで明示します。",
        Review,
        Some(FixApplicability::Review),
        CHECKLIST,
        "項目<TAB>説明",
        "an appropriate indentation or layout annotation"
    ),
    rule!(
        codes::FORM_FEED,
        Layout,
        Warning,
        "Form feed requires page-break review",
        "改ページ位置の確認",
        "A page break is represented by the appropriate Aozora annotation.",
        "改ページは適切な青空文庫注記で表します。",
        Review,
        Some(FixApplicability::Review),
        CHECKLIST,
        "<FF>",
        "［＃改ページ］"
    ),
    rule!(
        codes::ASCII_RUBY_MARKER,
        Ruby,
        Error,
        "ASCII ruby boundary marker",
        "半角の被ルビ区切り記号",
        "A parser-confirmed ruby boundary uses the full-width vertical line.",
        "被ルビ文字列の区切りには全角の縦線を用います。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "|青空《あおぞら》",
        "｜青空《あおぞら》"
    ),
    rule!(
        codes::ITERATION_MARK,
        Character,
        Error,
        "Non-canonical voiced iteration mark",
        "濁点付き二倍踊り字の誤記",
        "The established Aozora spelling uses a double-prime between the slashes.",
        "濁点付きくの字点は斜線の間に秒記号を置く規定形を使います。",
        Automatic,
        Some(FixApplicability::Safe),
        CHECKLIST,
        "／〃＼",
        "／″＼"
    ),
    rule!(
        codes::MODERN_CANDIDATE,
        Orthography,
        Note,
        "Traditional form under modern policy",
        "新字体方針にある旧字体",
        "The selected policy asks a reviewer to consider the recorded modern form.",
        "選択した方針に従い、記録された新字体候補を確認します。",
        Review,
        Some(FixApplicability::Review),
        PROOFREADING,
        "廣島",
        "広島"
    ),
    rule!(
        codes::TRADITIONAL_CANDIDATE,
        Orthography,
        Note,
        "Modern form under traditional policy",
        "旧字体方針にある新字体",
        "The selected policy asks a reviewer to choose among recorded traditional forms.",
        "選択した方針に従い、複数を含む旧字体候補を確認します。",
        Review,
        Some(FixApplicability::Review),
        PROOFREADING,
        "仏",
        "佛"
    ),
    rule!(
        codes::OCR_SIMILAR,
        Character,
        Warning,
        "OCR-confusable character",
        "OCR 類似字",
        "The character appears in a script context associated with OCR substitutions.",
        "OCR で取り違えやすい文字が不自然な文字種の並びにあります。",
        Review,
        None,
        PROOFREADING,
        "片仮名のタ and 漢字の夕",
        "the scalar present in the base edition"
    ),
    rule!(
        codes::SPACING,
        Layout,
        Note,
        "Context-dependent spacing or punctuation",
        "空白・約物の文脈確認",
        "ASCII spacing and punctuation next to Japanese text can require editorial interpretation.",
        "和文に接する半角空白や約物は底本と文脈を確認します。",
        Review,
        None,
        PROOFREADING,
        "和文 text",
        "the spacing in the base edition"
    ),
    rule!(
        codes::SMALL_KE,
        Orthography,
        Note,
        "ケ or ヶ requires review",
        "ケ・ヶの確認",
        "The choice between these forms depends on the base spelling.",
        "ケとヶの選択は底本の綴りに依存します。",
        Review,
        None,
        PROOFREADING,
        "ケ / ヶ",
        "the form in the base edition"
    ),
    rule!(
        codes::RUBY_GROUPING,
        Ruby,
        Warning,
        "Ruby grouping requires review",
        "ルビのまとまり確認",
        "Word boundaries and omitted readings determine whether ruby is over- or under-grouped.",
        "語境界や読みの省略により、ルビをまとめるか分けるかが変わります。",
        Review,
        Some(FixApplicability::Review),
        CHECKLIST,
        "青《あお》空《ぞら》",
        "青空《あおぞら》"
    ),
    rule!(
        codes::OPENING_LEGEND,
        Bibliography,
        Warning,
        "Opening symbol legend requires review",
        "冒頭の記号説明",
        "The opening legend must describe the symbols actually used by the work.",
        "冒頭の記号説明は作品中で実際に用いる記号と初出例に合わせます。",
        Review,
        Some(FixApplicability::Review),
        CHECKLIST,
        "body starts immediately",
        "テキスト中に現れる記号について"
    ),
    rule!(
        codes::CLOSING_BIBLIOGRAPHY,
        Bibliography,
        Warning,
        "Closing bibliography requires review",
        "末尾の書誌情報",
        "Submission text ends with complete, applicable bibliographical matter.",
        "提出テキストの末尾には該当する書誌情報を過不足なく置きます。",
        Review,
        Some(FixApplicability::Review),
        CHECKLIST,
        "body ends without source information",
        "底本：…"
    ),
    rule!(
        codes::MANUAL_BASE_TEXT,
        Manual,
        Error,
        "Compare every character with the base edition",
        "底本との一字一句照合",
        "No text-only tool can establish fidelity without the base edition.",
        "底本なしにテキストだけで一字一句の一致は確認できません。",
        Manual,
        None,
        PROOFREADING,
        "unchecked transcription",
        "independent repeated comparison"
    ),
    rule!(
        codes::MANUAL_LAYOUT,
        Manual,
        Warning,
        "Resolve semantic layout",
        "意味に依存するレイアウト判断",
        "Some indentation, tables, and page layout require the page image and meaning.",
        "字下げ、表、ページ配置の一部は紙面と意味を見て判断します。",
        Manual,
        None,
        PROOFREADING,
        "layout inferred from plain text",
        "layout confirmed against the edition"
    ),
    rule!(
        codes::MANUAL_RIGHTS,
        Manual,
        Error,
        "Resolve rights and bibliographical judgement",
        "著作権・書誌判断",
        "Copyright status and uncertain bibliographical claims require a responsible person.",
        "著作権状態と不確かな書誌事項は人が根拠を確認します。",
        Manual,
        None,
        CHECKLIST,
        "unverified publication claim",
        "documented rights and bibliographical decision"
    ),
    rule!(
        codes::MANUAL_FILENAME,
        Manual,
        Warning,
        "Confirm the submission filename",
        "提出ファイル名の確認",
        "The filename depends on the work identifier and submission record outside the text.",
        "ファイル名は本文外の作品識別情報と提出記録に照らして確認します。",
        Manual,
        None,
        CHECKLIST,
        "unverified filename",
        "filename confirmed against the submission record"
    ),
    rule!(
        codes::MANUAL_RESOURCE_FORK,
        Manual,
        Warning,
        "Remove host-specific file metadata",
        "リソースフォーク等の除去",
        "Host-specific metadata is outside the document byte stream inspected by the engine.",
        "リソースフォーク等は本文バイト列の外にあり、エンジンだけでは確認できません。",
        Manual,
        None,
        CHECKLIST,
        "archive with a resource fork",
        "plain submission file without host metadata"
    ),
];

/// One official checklist or manual requirement and its implemented coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfficialItem {
    /// Stable catalog identity.
    pub id: &'static str,
    /// Canonical English label.
    pub title: &'static str,
    /// Japanese label.
    pub title_ja: &'static str,
    /// Evaluation class.
    pub detection: DetectionClass,
    /// Rules that expose the requirement.
    pub rules: &'static [&'static str],
    /// Official source.
    pub authority_url: &'static str,
}

const OFFICIAL_ITEMS: &[OfficialItem] = &[
    OfficialItem {
        id: "checklist.filename",
        title: "Submission filename",
        title_ja: "提出ファイル名",
        detection: DetectionClass::Manual,
        rules: &[codes::MANUAL_FILENAME],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.windows-format",
        title: "Shift_JIS, JIS X 0208, and CRLF",
        title_ja: "Shift_JIS・JIS X 0208・CRLF",
        detection: DetectionClass::Automatic,
        rules: &[
            codes::SOURCE_ENCODING,
            codes::LINE_ENDING,
            codes::BOM,
            codes::INVALID_ENCODING,
            codes::HALFWIDTH_KANA,
            codes::PLATFORM_DEPENDENT,
            codes::NEEDS_GAIJI,
            codes::CONTROL,
        ],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.resource-fork",
        title: "Resource fork removal",
        title_ja: "リソースフォークの除去",
        detection: DetectionClass::Manual,
        rules: &[codes::MANUAL_RESOURCE_FORK],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.opening-legend",
        title: "Opening symbol legend",
        title_ja: "冒頭の「記号について」",
        detection: DetectionClass::Review,
        rules: &[codes::OPENING_LEGEND],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.closing-bibliography",
        title: "Closing bibliographical matter",
        title_ja: "末尾の書誌情報",
        detection: DetectionClass::Review,
        rules: &[codes::CLOSING_BIBLIOGRAPHY],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.final-newline",
        title: "Final line ending",
        title_ja: "末尾の終端改行",
        detection: DetectionClass::Automatic,
        rules: &[codes::FINAL_NEWLINE],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.ruby-grouping",
        title: "Ruby grouping",
        title_ja: "ルビのまとまり",
        detection: DetectionClass::Review,
        rules: &[codes::RUBY_GROUPING],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.ruby-boundary",
        title: "Ruby boundary marker",
        title_ja: "被ルビ区切り記号",
        detection: DetectionClass::Automatic,
        rules: &[codes::ASCII_RUBY_MARKER],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.gaiji",
        title: "External-character annotations",
        title_ja: "外字注記",
        detection: DetectionClass::Automatic,
        rules: &[codes::NEEDS_GAIJI, codes::PLATFORM_DEPENDENT],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "checklist.iteration-mark",
        title: "Double iteration mark",
        title_ja: "二倍の踊り字",
        detection: DetectionClass::Automatic,
        rules: &[codes::ITERATION_MARK],
        authority_url: CHECKLIST,
    },
    OfficialItem {
        id: "manual.base-comparison",
        title: "Repeated comparison with the base edition",
        title_ja: "底本との反復照合",
        detection: DetectionClass::Manual,
        rules: &[codes::MANUAL_BASE_TEXT],
        authority_url: PROOFREADING,
    },
    OfficialItem {
        id: "manual.ocr",
        title: "OCR-confusable characters",
        title_ja: "OCR 類似字",
        detection: DetectionClass::Review,
        rules: &[codes::OCR_SIMILAR],
        authority_url: PROOFREADING,
    },
    OfficialItem {
        id: "manual.orthography",
        title: "Modern and traditional forms",
        title_ja: "新旧字体",
        detection: DetectionClass::Review,
        rules: &[
            codes::MODERN_CANDIDATE,
            codes::TRADITIONAL_CANDIDATE,
            codes::SMALL_KE,
        ],
        authority_url: PROOFREADING,
    },
    OfficialItem {
        id: "manual.layout-candidates",
        title: "Spacing and explicit layout candidates",
        title_ja: "空白と明示的なレイアウト候補",
        detection: DetectionClass::Review,
        rules: &[codes::SPACING, codes::TAB, codes::FORM_FEED],
        authority_url: PROOFREADING,
    },
    OfficialItem {
        id: "manual.layout",
        title: "Meaning-dependent layout",
        title_ja: "意味に依存するレイアウト",
        detection: DetectionClass::Manual,
        rules: &[codes::MANUAL_LAYOUT],
        authority_url: PROOFREADING,
    },
    OfficialItem {
        id: "manual.rights",
        title: "Copyright and bibliographical judgement",
        title_ja: "著作権と書誌判断",
        detection: DetectionClass::Manual,
        rules: &[codes::MANUAL_RIGHTS],
        authority_url: CHECKLIST,
    },
];

/// Look up a rule by stable code.
#[must_use]
pub fn explain(code: &str) -> Option<RuleDoc> {
    RULES.iter().copied().find(|rule| rule.code == code)
}

/// Complete ordered catalog.
#[must_use]
pub const fn all_rules() -> &'static [RuleDoc] {
    RULES
}

/// Official coverage table.
#[must_use]
pub const fn official_items() -> &'static [OfficialItem] {
    OFFICIAL_ITEMS
}

/// Default metadata for an upstream parser diagnostic.
#[must_use]
pub const fn upstream_rule() -> RuleDoc {
    rule!(
        "",
        Notation,
        Warning,
        "Aozora notation diagnostic",
        "青空文庫記法の指摘",
        "The upstream parser found a notation issue.",
        "上流 parser が青空文庫記法の問題を検出しました。",
        Automatic,
        None,
        AOZORA_PARSER,
        "malformed notation",
        "well-formed notation"
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn codes_and_official_item_ids_are_unique() {
        let mut codes = HashSet::new();
        for rule in all_rules() {
            assert!(codes.insert(rule.code), "duplicate rule {}", rule.code);
        }
        let mut ids = HashSet::new();
        for item in official_items() {
            assert!(ids.insert(item.id), "duplicate official item {}", item.id);
        }
    }

    #[test]
    fn every_official_item_has_catalog_coverage() {
        let mut covered = HashSet::new();
        for item in official_items() {
            assert!(!item.rules.is_empty(), "{} has no mapped rule", item.id);
            for code in item.rules {
                let rule = explain(code);
                assert!(rule.is_some(), "{} maps unknown rule {code}", item.id);
                assert_eq!(
                    rule.map(|value| value.detection),
                    Some(item.detection),
                    "{} classifies {code} inconsistently",
                    item.id
                );
                covered.insert(*code);
            }
        }
        for rule in all_rules() {
            assert!(covered.contains(rule.code), "{} is not mapped", rule.code);
        }
    }

    #[test]
    fn manual_rules_never_claim_a_fix() {
        for rule in all_rules() {
            if rule.detection == DetectionClass::Manual {
                assert!(rule.fix.is_none(), "{} offers a manual fix", rule.code);
            }
        }
    }

    #[test]
    fn owned_codes_use_proof_namespace() {
        assert!(
            all_rules()
                .iter()
                .all(|rule| rule.code.starts_with("aozora::proof::"))
        );
    }
}

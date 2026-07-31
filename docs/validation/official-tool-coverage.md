# Official-tool coverage

This matrix defines the validation baseline. It is not a claim of endorsement
or replacement.

Primary references:

- [青空文庫作業マニュアル 入力編](https://www.aozora.gr.jp/aozora-manual/index-input.html)
- [青空文庫作業マニュアル 校正編](https://www.aozora.gr.jp/aozora-manual/index-proofreading.html)
- [入力ファイルを「テキスト版」に仕上げるために](https://www.aozora.gr.jp/KOSAKU/textfile_checklist/)
- [青空文庫 耕作員手帳](https://www.aozora.gr.jp/guide/techo.html)

| Area | Existing workflow | aozora-proof status | Validation stance |
|---|---|---|---|
| JIS X 0208 / 機種依存文字 | 文字チェッカー、チェッカー君 | Implemented | Compare findings; do not copy implementation data |
| 半角カナ領域 | チェッカー君 | Implemented; letters and punctuation reported separately | Objective default rule |
| タブ・改ページ・その他の制御文字 | Checker tools and manual | Implemented as separate actionable findings | Do not silently delete layout intent |
| 改行・保存形式 | Manual and editor settings | Implemented | Submission-only notes for UTF-8 and non-CRLF |
| 旧字・新字 | 校閲君 | Implemented as Note | Requires confirmation against the source book |
| 外字注記 | 外字注記辞書 and search tools | Lookup and suggestions implemented | Suggestions remain read-only |
| 青空文庫記法 | Existing conversion/checking tools | Delegated to `aozora` | Consume parser diagnostics and original-source spans |
| 半角空白・半角括弧 | Checker tools | Not a default rule | Context-sensitive experiment only |
| OCR confusables | 校正ツール and manual searches | Not implemented | Requires source-book confirmation; not an objective default |
| Difference/history reports | 相違点・修正履歴 tools | Not implemented | Outside the first validation campaign |

External checker implementations are comparison oracles. Their code, rule
tables, and configuration are not vendored unless their licensing is separately
reviewed and compatible.

See [External oracle comparison](external-oracle-comparison.md) for the current
behavioral boundary and [Public corpus baseline](public-corpus-baseline.md) for
the measured result.

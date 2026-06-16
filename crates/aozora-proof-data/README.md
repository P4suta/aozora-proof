# aozora-proof-data

Character-classification tables for [`aozora-proof`](../../README.md).

Bakes the [Project X0213](https://x0213.org/codetable/) mapping table
(`data/jisx0213-2004-std.txt`, freely redistributable) into a compile-time
classifier:

- `jis_level(char) -> Suijun` — 第1 / 第2 / 第3 / 第4 水準, or Outside.
  第1∪第2水準 = JIS X 0208 = usable literally in conformant Aozora text.
- `is_platform_dependent(char) -> bool` — 機種依存文字 (CP932 ∖ JIS X 0208).

`forbid(unsafe_code)`, WASM-clean. See [`NOTICE`](../../NOTICE) for data
provenance and licensing.

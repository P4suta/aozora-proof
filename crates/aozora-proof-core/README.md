# aozora-proof-core

The character-level proofreading engine for 青空文庫記法 (Aozora Bunko notation)
text — the pure, WASM-clean library behind `aozora-proof`.

It consumes the [`aozora`](https://github.com/P4suta/aozora) parser for the
notation level and adds the character level (JIS X 0208 conformance,
機種依存文字, 旧字体↔新字体, half/full-width, file structure), merging both into
one unified, JSON-serialisable `Report`.

See the [repository README](../../README.md) for the full picture.

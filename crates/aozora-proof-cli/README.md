# aozora-proof-cli

The `aozora-proof` command-line proofreader for 青空文庫 text.

```console
$ aozora-proof check seihon.txt
$ cat seihon.txt | aozora-proof check -
$ aozora-proof check --format json *.txt          # machine-readable, for CI
$ aozora-proof check --fail-on warning chapter*.txt
```

Runs the [`aozora`](https://github.com/P4suta/aozora) parser's notation
diagnostics plus the character-level checks (JIS X 0208 conformance,
機種依存文字, BOM / line endings) and merges them into one report.

## Exit codes

| code | meaning |
|---|---|
| `0` | clean (or findings below `--fail-on`) |
| `1` | `--strict` set with any finding, or a finding at/above `--fail-on` |
| `2` | usage / input error (bad arguments, unreadable file) |
| `3` | an internal-source finding fired (a tool bug — report upstream) |

`--format auto` (default) prints human-readable output on a terminal and the
JSON envelope when piped.

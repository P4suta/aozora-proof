# aozora-proof-cli

Private implementation crate for the `aozora-proof` binary. It provides
submission `check`, safe `fix`, interactive `review`, reference commands,
schema-v2 JSON and SARIF, deterministic discovery, layered configuration, and
generated man/completion output.

```console
$ aozora-proof check --orthography mixed manuscript.txt
$ aozora-proof fix --orthography mixed --dry-run manuscript.txt
$ aozora-proof review --orthography traditional manuscript.txt
$ aozora-proof rules
```

Exit codes are `0` success, `1` configured finding threshold reached, `2`
usage/I/O/decode/configuration/write failure, and `3` internal invariant
failure. A closed output pipe is success.

This crate is not a supported Rust API and is not published to crates.io. See
the [repository README](../../README.md) for the supported binary interface.

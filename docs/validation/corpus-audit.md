# Corpus audit protocol

Run the unpublished audit command against a directory containing released
plain-text works:

```console
just audit-corpus ../aozorabunko_text
```

The command recursively checks `.txt` files and writes deterministic JSON to
`target/corpus-audit.json`. The report contains aggregate counts, rule totals,
affected-file totals, and a bounded set of relative sample paths. It contains
no source excerpts and no timestamps.

Review disagreements with external checkers as one of:

1. aozora-proof defect;
2. external-checker defect;
3. intentional scope difference;
4. unresolved and therefore ineligible for a default rule.

A default Error or Warning must have no unexplained false positives in the
reviewed sample. Experimental rules stay outside `run_submission` until they
meet that condition.

Private or pre-publication files may be scanned only from an explicitly
provided local directory. Do not commit inputs, excerpts, absolute paths, or
reports containing identifying filenames.

The recorded public-corpus result is in
[Public corpus baseline](public-corpus-baseline.md). Independent behavior is
tracked in [External oracle comparison](external-oracle-comparison.md).

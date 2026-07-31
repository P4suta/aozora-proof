# Release runbook

The repository releases one product: the native CLI, its composite GitHub
Action, and the Web/Pages site built from the same commit. Rust crates are not
published.

## One-time GitHub setup

These are the only settings not applied by repository code:

1. Create the `release-please` Environment. Allow only `main`, require no
   reviewer, and add `RELEASE_PLEASE_APP_ID` and
   `RELEASE_PLEASE_APP_PRIVATE_KEY`. Store the GitHub App **Client ID** (the
   current recommended identifier; the secret name is retained for interface
   compatibility) in `RELEASE_PLEASE_APP_ID`.
2. Grant that GitHub App Contents, Pull requests, and Issues read/write access
   to this repository. Do not create a PAT.
3. Create the `release` Environment. Allow only tags matching `v*`, require
   reviewer `P4suta`, allow self-review, and add no secrets.
4. Apply [the main ruleset](../.github/rulesets/main.json) and
   [the tag ruleset](../.github/rulesets/tags.json) with the repository ruleset
   API or GitHub settings.
5. Enable release immutability in Settings → General → Releases.

After setup, run:

```console
$ just release-preflight --repository P4suta/aozora-proof --commit "$(git rev-parse HEAD)"
```

## Normal release

Release Please maintains a PR from Conventional Commits. Its App-authenticated
workflow synchronizes generated version files, disables auto-merge, and leaves
the PR for a manual squash merge.

Review the version and generated changelog, wait for `ci-success`,
`release-ready`, `codeql`, and `dependency-review`, then squash-merge manually.
The merge commit is qualified again. Five native builds, smoke tests, archives,
checksums, SPDX SBOMs, and the aggregate manifest are saved under that exact
commit.

Only the successful main push dispatches `release.yml`. It creates `vX.Y.Z`
and a complete draft, attaches and attests every archive, then dispatches the
tag-scoped `publish-release.yml`. Approve its `release` Environment deployment.
The job revalidates everything, publishes the draft, and dispatches the Pages
deployment for the same commit.

## Recovery and dry run

Never recover by pushing a tag. Dispatch `release.yml` and provide the exact
qualified commit. Leave `dry_run` enabled first:

```console
$ gh workflow run release.yml --ref main \
    --field commit=0123456789abcdef0123456789abcdef01234567 \
    --field dry_run=true
```

If verification succeeds, rerun with `dry_run=false`. Repeating the same
commit/tag is idempotent: the workflow verifies an existing tag, reuses the
draft, replaces draft assets only before publication, and treats an already
published verified release as complete.

If `release-ready` failed, there is no qualified aggregate artifact and
recovery stops before any tag or draft is created. After merging an
infrastructure-only fix, explicitly requalify that exact commit on `main`:

```console
$ gh workflow run release-ready.yml --ref main \
    --field commit=0123456789abcdef0123456789abcdef01234567
```

Manual qualification accepts only a full commit SHA contained in `main` and
does not dispatch publication automatically. Once it succeeds, use the
`release.yml` dry-run recovery above.

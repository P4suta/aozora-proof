#!/usr/bin/env bash
# Commit Release Please's derived files with GitHub's signed-commit API.
set -euo pipefail

if git diff --quiet; then
    echo "release files are already synchronized"
    exit 0
fi

mapfile -t changed < <(git diff --name-only)
allowed=(
    Cargo.lock
    Cargo.toml
    README.md
    fuzz/Cargo.lock
)
for path in "${changed[@]}"; do
    permitted=false
    for candidate in "${allowed[@]}"; do
        if [[ "$path" == "$candidate" ]]; then
            permitted=true
            break
        fi
    done
    if [[ "$permitted" != "true" ]]; then
        echo "release sync changed unexpected path: $path" >&2
        exit 1
    fi
done

additions='[]'
for path in "${changed[@]}"; do
    if base64 --help >/dev/null 2>&1; then
        contents=$(base64 --wrap=0 "$path")
    else
        contents=$(base64 <"$path" | tr -d '\r\n')
    fi
    additions=$(jq \
        --arg path "$path" \
        --arg contents "$contents" \
        '. + [{path: $path, contents: $contents}]' \
        <<<"$additions")
done

head_oid=$(git rev-parse HEAD)
input=$(jq --null-input \
    --arg repository "$GITHUB_REPOSITORY" \
    --arg branch "$BRANCH" \
    --arg expected "$head_oid" \
    --argjson additions "$additions" \
    '{
      branch: {
        repositoryNameWithOwner: $repository,
        branchName: $branch
      },
      message: {
        headline: "chore(release): synchronize generated versions"
      },
      fileChanges: {
        additions: $additions
      },
      expectedHeadOid: $expected
    }')

# GraphQL variables intentionally retain the literal `$input`.
# shellcheck disable=SC2016
query='mutation($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit { oid url }
  }
}'
payload=$(jq --null-input --arg query "$query" --argjson input "$input" \
    '{query: $query, variables: {input: $input}}')
response=$(gh api graphql --input - <<<"$payload")
commit=$(jq --raw-output '.data.createCommitOnBranch.commit.oid // empty' <<<"$response")
if [[ -z "$commit" ]]; then
    echo "GitHub did not create the release synchronization commit" >&2
    jq . <<<"$response" >&2
    exit 1
fi
echo "created signed synchronization commit $commit"

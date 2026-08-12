#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <run-id> <source-commit> <candidate-branch> [repository]" >&2
  exit 64
}

fail() {
  echo "check-github-run: $1" >&2
  exit 1
}

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  usage
fi

run_id=$1
source_commit=$2
candidate_branch=$3
repository=${4:-heurema/goalrail-rs}

if ! printf '%s\n' "$run_id" | grep -Eq '^[1-9][0-9]*$'; then
  fail "run ID must be a positive integer"
fi
if ! printf '%s\n' "$source_commit" | grep -Eq '^[0-9a-f]{40}$'; then
  fail "source commit must be a full Git SHA"
fi
if ! printf '%s\n' "$candidate_branch" |
  grep -Eq '^release-candidate/v[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "candidate branch must use release-candidate/vX.Y.Z"
fi
if ! printf '%s\n' "$repository" |
  grep -Eq '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$'; then
  fail "repository must use owner/name"
fi

command -v gh >/dev/null 2>&1 || fail "gh is unavailable"
command -v jq >/dev/null 2>&1 || fail "jq is unavailable"

run_json=$(gh api "repos/$repository/actions/runs/$run_id") ||
  fail "GitHub Actions run could not be read: $repository $run_id"

printf '%s\n' "$run_json" | jq -e \
  --arg run_id "$run_id" \
  --arg source_commit "$source_commit" \
  --arg candidate_branch "$candidate_branch" \
  --arg repository "$repository" '
    (.id | tostring) == $run_id
    and .name == "build release candidate"
    and .path == ".github/workflows/release.yml"
    and .event == "workflow_dispatch"
    and .head_branch == $candidate_branch
    and .head_sha == $source_commit
    and .status == "completed"
    and .conclusion == "success"
    and (.run_attempt | type == "number" and . > 0)
    and .repository.full_name == $repository
  ' >/dev/null || fail "GitHub Actions run identity or outcome does not match"

run_attempt=$(printf '%s\n' "$run_json" | jq -er '.run_attempt | tostring')
url=$(printf '%s\n' "$run_json" | jq -er '.html_url')
jq -cn \
  --arg repository "$repository" \
  --arg run_id "$run_id" \
  --arg run_attempt "$run_attempt" \
  --arg source_commit "$source_commit" \
  --arg candidate_branch "$candidate_branch" \
  --arg url "$url" '{
    schemaVersion: 1,
    verdict: "RUN_VERIFIED",
    repository: $repository,
    runId: $run_id,
    runAttempt: $run_attempt,
    sourceCommit: $source_commit,
    candidateBranch: $candidate_branch,
    url: $url
  }'

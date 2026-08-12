#!/bin/sh

set -u

usage() {
  echo "Usage: $0 <version> <run-id> <run-attempt> <tap-root>" >&2
  exit 64
}

plain_blocked() {
  echo "promote-homebrew: $1" >&2
  exit 4
}

blocked() {
  code=$1
  message=$2
  jq -cn --arg code "$code" --arg message "$message" '{
    schemaVersion: 1,
    verdict: "CONFLICT",
    finding: {code: $code, message: $message}
  }'
  exit 4
}

checksum() {
  file=$1

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    return 1
  fi
}

remote_main() {
  remote_refs=$(GIT_SSH_COMMAND='ssh -o BatchMode=yes' \
    git ls-remote --exit-code "$canonical_push" refs/heads/main 2>/dev/null) ||
    return 1
  remote_count=$(printf '%s\n' "$remote_refs" |
    awk '$2 == "refs/heads/main" { count += 1 } END { print count + 0 }')
  [ "$remote_count" -eq 1 ] || return 1
  printf '%s\n' "$remote_refs" |
    awk '$2 == "refs/heads/main" { print $1 }'
}

semver_gt() {
  awk -v candidate="$1" -v current="$2" 'BEGIN {
    split(candidate, a, ".")
    split(current, b, ".")
    for (i = 1; i <= 3; i += 1) {
      if (length(a[i]) > length(b[i])) exit 0
      if (length(a[i]) < length(b[i])) exit 1
      if (("x" a[i]) > ("x" b[i])) exit 0
      if (("x" a[i]) < ("x" b[i])) exit 1
    }
    exit 1
  }'
}

[ "$#" -eq 4 ] || usage
version=$1
run_id=$2
run_attempt=$3
tap_argument=$4

printf '%s\n' "$version" |
  grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' || usage
printf '%s\n' "$run_id" | grep -Eq '^[1-9][0-9]*$' || usage
printf '%s\n' "$run_attempt" | grep -Eq '^[1-9][0-9]*$' || usage

for command in git gh jq curl; do
  command -v "$command" >/dev/null 2>&1 || plain_blocked "$command is unavailable"
done

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd -P)
cd "$repo_root" || plain_blocked "source repository is unavailable"

tag="v$version"
candidate_branch="release-candidate/$tag"
release_dir="$repo_root/dist/$tag"
release_manifest="$release_dir/release.json"
formula="$release_dir/goalrail.rb"
archive="$release_dir/goalrail-$tag-aarch64-apple-darwin.tar.gz"
canonical_fetch=https://github.com/heurema/homebrew-tap.git
canonical_push=git@github.com:heurema/homebrew-tap.git
formula_path=Formula/goalrail.rb
commit_subject="Promote Goalrail $tag"

for file in "$release_manifest" "$formula" "$archive"; do
  [ -f "$file" ] || blocked "BUNDLE_MISSING" "verified release input is missing: $file"
done

source_commit=$(jq -er '.sourceCommit' "$release_manifest" 2>/dev/null) ||
  blocked "MANIFEST_INVALID" "release manifest omits sourceCommit"
manifest_version=$(jq -er '.version' "$release_manifest" 2>/dev/null) ||
  blocked "MANIFEST_INVALID" "release manifest omits version"
manifest_run_id=$(jq -er '.run.id' "$release_manifest" 2>/dev/null) ||
  blocked "MANIFEST_INVALID" "release manifest omits run ID"
manifest_run_attempt=$(jq -er '.run.attempt' "$release_manifest" 2>/dev/null) ||
  blocked "MANIFEST_INVALID" "release manifest omits run attempt"
[ "$manifest_version" = "$version" ] ||
  blocked "MANIFEST_MISMATCH" "release manifest version differs from the request"
[ "$manifest_run_id" = "$run_id" ] ||
  blocked "MANIFEST_MISMATCH" "release manifest run ID differs from the request"
[ "$manifest_run_attempt" = "$run_attempt" ] ||
  blocked "MANIFEST_MISMATCH" "release manifest run attempt differs from the request"
printf '%s\n' "$source_commit" | grep -Eq '^[0-9a-f]{40}$' ||
  blocked "MANIFEST_INVALID" "release manifest sourceCommit is not a full Git SHA"

[ -z "$(git status --porcelain)" ] ||
  blocked "SOURCE_DIRTY" "release source checkout is not clean"
[ "$(git rev-parse HEAD 2>/dev/null)" = "$source_commit" ] ||
  blocked "SOURCE_MISMATCH" "release source checkout is not at the manifest commit"
tag_commit=$(git rev-parse "$tag^{}" 2>/dev/null) ||
  blocked "TAG_MISSING" "release tag is unavailable locally"
[ "$tag_commit" = "$source_commit" ] ||
  blocked "TAG_MISMATCH" "release tag does not resolve to the manifest commit"

run_receipt=$("$script_dir/check-github-run.sh" \
  "$run_id" "$source_commit" "$candidate_branch" heurema/goalrail-rs 2>/dev/null) ||
  blocked "RUN_UNVERIFIED" "selected GitHub Actions run did not verify"
printf '%s\n' "$run_receipt" | jq -e --arg attempt "$run_attempt" '
  .verdict == "RUN_VERIFIED" and .runAttempt == $attempt
' >/dev/null 2>&1 ||
  blocked "RUN_MISMATCH" "selected GitHub Actions run attempt differs from the request"

"$script_dir/check-release-bundle.sh" \
  "$version" "$source_commit" "$run_id" "$run_attempt" "$repo_root/dist" \
  >/dev/null 2>&1 ||
  blocked "BUNDLE_UNVERIFIED" "release bundle did not verify"
"$script_dir/check-github-release.sh" \
  "$version" published "$run_id" "$run_attempt" "$repo_root/dist" origin \
  >/dev/null 2>&1 ||
  blocked "PUBLIC_RELEASE_UNVERIFIED" "published GitHub Release did not verify"

formula_url="https://github.com/heurema/goalrail-rs/releases/download/$tag/goalrail-$tag-aarch64-apple-darwin.tar.gz"
grep -Fqx "  url \"$formula_url\"" "$formula" ||
  blocked "FORMULA_URL_MISMATCH" "formula does not contain the exact release archive URL"
formula_sha=$(sed -n 's/^[[:space:]]*sha256 "\([0-9a-f][0-9a-f]*\)"$/\1/p' "$formula")
printf '%s\n' "$formula_sha" | grep -Eq '^[0-9a-f]{64}$' ||
  blocked "FORMULA_SHA_INVALID" "formula must contain one SHA-256 digest"

downloaded_archive=$(mktemp "${TMPDIR:-/tmp}/goalrail-homebrew-archive.XXXXXX") ||
  blocked "TEMP_UNAVAILABLE" "temporary archive storage could not be created"
remote_formula=$(mktemp "${TMPDIR:-/tmp}/goalrail-homebrew-formula.XXXXXX") || {
  rm -f "$downloaded_archive"
  blocked "TEMP_UNAVAILABLE" "temporary formula storage could not be created"
}
post_formula=$(mktemp "${TMPDIR:-/tmp}/goalrail-homebrew-post.XXXXXX") || {
  rm -f "$downloaded_archive" "$remote_formula"
  blocked "TEMP_UNAVAILABLE" "temporary verification storage could not be created"
}
cleanup() {
  rm -f "$downloaded_archive" "$remote_formula" "$post_formula"
}
trap cleanup EXIT HUP INT TERM

curl --fail --silent --show-error --location \
  --output "$downloaded_archive" "$formula_url" ||
  blocked "PUBLIC_ARCHIVE_UNAVAILABLE" "public release archive could not be downloaded"
downloaded_sha=$(checksum "$downloaded_archive") ||
  blocked "CHECKSUM_TOOL_MISSING" "no SHA-256 command is available"
[ "$downloaded_sha" = "$formula_sha" ] ||
  blocked "PUBLIC_ARCHIVE_MISMATCH" "public archive digest differs from the formula"
[ "$(checksum "$archive")" = "$formula_sha" ] ||
  blocked "LOCAL_ARCHIVE_MISMATCH" "local archive digest differs from the formula"

[ -d "$tap_argument" ] || blocked "TAP_MISSING" "tap root is not a directory"
tap_root=$(CDPATH='' cd -- "$tap_argument" && pwd -P) ||
  blocked "TAP_MISSING" "tap root could not be resolved"
tap_toplevel=$(git -C "$tap_root" rev-parse --show-toplevel 2>/dev/null) ||
  blocked "TAP_INVALID" "tap root is not a Git checkout"
tap_toplevel=$(CDPATH='' cd -- "$tap_toplevel" && pwd -P) ||
  blocked "TAP_INVALID" "tap Git toplevel could not be resolved"
[ "$tap_root" = "$tap_toplevel" ] ||
  blocked "TAP_SCOPE_MISMATCH" "tap argument must be the exact Git toplevel"
[ "$(git -C "$tap_root" symbolic-ref --short HEAD 2>/dev/null)" = main ] ||
  blocked "TAP_BRANCH_MISMATCH" "tap checkout must be on main"
tap_origin=$(git -C "$tap_root" config --get remote.origin.url 2>/dev/null) ||
  blocked "TAP_ORIGIN_MISSING" "tap checkout has no origin URL"
case "$tap_origin" in
  https://github.com/heurema/homebrew-tap|https://github.com/heurema/homebrew-tap.git|git@github.com:heurema/homebrew-tap.git) ;;
  *) blocked "TAP_IDENTITY_MISMATCH" "tap origin is not heurema/homebrew-tap" ;;
esac

permission=$(gh api repos/heurema/homebrew-tap --jq '.permissions.push' 2>/dev/null) ||
  blocked "WRITE_PERMISSION_UNVERIFIED" "GitHub tap write permission could not be read"
[ "$permission" = true ] ||
  blocked "WRITE_PERMISSION_MISSING" "GitHub does not report tap push permission"

remote_head=$(remote_main) ||
  blocked "REMOTE_UNAVAILABLE" "canonical tap main could not be read over SSH"
printf '%s\n' "$remote_head" | grep -Eq '^[0-9a-f]{40}$' ||
  blocked "REMOTE_AMBIGUOUS" "canonical tap main is missing or ambiguous"

git -C "$tap_root" fetch --quiet "$canonical_fetch" refs/heads/main ||
  blocked "REMOTE_FETCH_FAILED" "canonical tap main could not be fetched"
[ "$(git -C "$tap_root" rev-parse FETCH_HEAD 2>/dev/null)" = "$remote_head" ] ||
  blocked "REMOTE_FETCH_MISMATCH" "fetched tap head differs from the observed head"
git -C "$tap_root" show "$remote_head:$formula_path" >"$remote_formula" 2>/dev/null ||
  blocked "REMOTE_FORMULA_MISSING" "remote tap formula is unavailable"

local_head=$(git -C "$tap_root" rev-parse HEAD 2>/dev/null) ||
  blocked "TAP_INVALID" "tap HEAD could not be resolved"
tap_status=$(git -C "$tap_root" status --porcelain) ||
  blocked "TAP_STATUS_FAILED" "tap worktree state could not be read"

if cmp -s "$formula" "$remote_formula"; then
  [ "$local_head" = "$remote_head" ] && [ -z "$tap_status" ] ||
    blocked "LOCAL_STATE_CONFLICT" "remote formula is current but the local tap is not clean at remote main"
  jq -cn --arg version "$version" --arg remoteHead "$remote_head" '{
    schemaVersion: 1,
    verdict: "NO_CHANGE",
    version: $version,
    remoteHead: $remoteHead
  }'
  exit 0
fi

remote_version=$(sed -n 's#^[[:space:]]*url "https://github.com/heurema/goalrail-rs/releases/download/v\([0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*\)/goalrail-v\1-aarch64-apple-darwin.tar.gz"$#\1#p' "$remote_formula")
printf '%s\n' "$remote_version" |
  grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' ||
  blocked "REMOTE_VERSION_INVALID" "remote formula version could not be resolved exactly"
[ "$remote_version" != "$version" ] ||
  blocked "SAME_VERSION_CONFLICT" "remote formula has the requested version with different bytes"
semver_gt "$version" "$remote_version" ||
  blocked "VERSION_NOT_NEWER" "requested version is not newer than the remote formula"

GIT_SSH_COMMAND='ssh -o BatchMode=yes' \
  git -C "$tap_root" push --dry-run "$canonical_push" \
  "$remote_head:refs/heads/main" >/dev/null 2>&1 ||
  blocked "PUSH_PREFLIGHT_FAILED" "exact SSH push transport failed before local mutation"

author_name=$(git show -s --format=%an "$source_commit") ||
  blocked "SOURCE_IDENTITY_MISSING" "release author name is unavailable"
author_email=$(git show -s --format=%ae "$source_commit") ||
  blocked "SOURCE_IDENTITY_MISSING" "release author email is unavailable"
case "$author_email" in
  *@users.noreply.github.com) ;;
  *) blocked "SOURCE_IDENTITY_INVALID" "release author must use a GitHub noreply email" ;;
esac
[ -n "$author_name" ] || blocked "SOURCE_IDENTITY_INVALID" "release author name is empty"

prepared_head=
if [ "$local_head" = "$remote_head" ]; then
  if [ -n "$tap_status" ]; then
    status_count=$(printf '%s\n' "$tap_status" | awk 'NF { count += 1 } END { print count + 0 }')
    [ "$status_count" -eq 1 ] ||
      blocked "LOCAL_STATE_CONFLICT" "tap has changes outside the exact formula resume state"
    case "$tap_status" in
      ' M Formula/goalrail.rb'|'M  Formula/goalrail.rb'|'MM Formula/goalrail.rb') ;;
      *) blocked "LOCAL_STATE_CONFLICT" "tap has an unsupported formula worktree state" ;;
    esac
    cmp -s "$tap_root/$formula_path" "$formula" ||
      blocked "LOCAL_STATE_CONFLICT" "prepared formula bytes differ from the release"
  else
    cp "$formula" "$tap_root/$formula_path" ||
      blocked "LOCAL_WRITE_FAILED" "verified formula could not be copied into the tap"
  fi
  git -C "$tap_root" add -- "$formula_path" ||
    blocked "LOCAL_STAGE_FAILED" "verified formula could not be staged"
  GIT_AUTHOR_NAME=$author_name GIT_AUTHOR_EMAIL=$author_email \
  GIT_COMMITTER_NAME=$author_name GIT_COMMITTER_EMAIL=$author_email \
    git -C "$tap_root" -c commit.gpgsign=false commit -m "$commit_subject" \
    >/dev/null ||
    blocked "LOCAL_COMMIT_FAILED" "formula promotion commit could not be created"
  prepared_head=$(git -C "$tap_root" rev-parse HEAD)
elif [ -z "$tap_status" ] &&
  git -C "$tap_root" merge-base --is-ancestor "$local_head" "$remote_head" 2>/dev/null; then
  git -C "$tap_root" merge --ff-only --quiet "$remote_head" ||
    blocked "LOCAL_FAST_FORWARD_FAILED" "tap could not fast-forward to remote main"
  cp "$formula" "$tap_root/$formula_path" ||
    blocked "LOCAL_WRITE_FAILED" "verified formula could not be copied into the tap"
  git -C "$tap_root" add -- "$formula_path" ||
    blocked "LOCAL_STAGE_FAILED" "verified formula could not be staged"
  GIT_AUTHOR_NAME=$author_name GIT_AUTHOR_EMAIL=$author_email \
  GIT_COMMITTER_NAME=$author_name GIT_COMMITTER_EMAIL=$author_email \
    git -C "$tap_root" -c commit.gpgsign=false commit -m "$commit_subject" \
    >/dev/null ||
    blocked "LOCAL_COMMIT_FAILED" "formula promotion commit could not be created"
  prepared_head=$(git -C "$tap_root" rev-parse HEAD)
elif [ -z "$tap_status" ] &&
  [ "$(git -C "$tap_root" rev-parse "$local_head^" 2>/dev/null)" = "$remote_head" ] &&
  [ "$(git -C "$tap_root" show -s --format=%s "$local_head")" = "$commit_subject" ] &&
  [ "$(git -C "$tap_root" show -s --format=%an "$local_head")" = "$author_name" ] &&
  [ "$(git -C "$tap_root" show -s --format=%ae "$local_head")" = "$author_email" ] &&
  [ "$(git -C "$tap_root" show -s --format=%cn "$local_head")" = "$author_name" ] &&
  [ "$(git -C "$tap_root" show -s --format=%ce "$local_head")" = "$author_email" ] &&
  [ "$(git -C "$tap_root" diff-tree --no-commit-id --name-only -r "$local_head")" = "$formula_path" ]; then
  git -C "$tap_root" show "$local_head:$formula_path" >"$post_formula" 2>/dev/null ||
    blocked "LOCAL_STATE_CONFLICT" "prepared formula commit has no formula blob"
  cmp -s "$post_formula" "$formula" ||
    blocked "LOCAL_STATE_CONFLICT" "prepared formula commit bytes differ from the release"
  prepared_head=$local_head
else
  blocked "LOCAL_STATE_CONFLICT" "tap is neither at remote main nor in an exact resumable state"
fi

git -C "$tap_root" show --stat --oneline "$prepared_head" >&2
git -C "$tap_root" diff --stat "$remote_head..$prepared_head" >&2

race_head=$(remote_main) ||
  blocked "REMOTE_UNAVAILABLE" "canonical tap main could not be reread before push"
[ "$race_head" = "$remote_head" ] ||
  blocked "REMOTE_RACE" "canonical tap main changed after preflight; push was not attempted"

GIT_SSH_COMMAND='ssh -o BatchMode=yes' \
  git -C "$tap_root" push "$canonical_push" \
  "$prepared_head:refs/heads/main" >/dev/null 2>&1 ||
  blocked "PUSH_REJECTED" "formula push was rejected; exact local commit is retained for inspection"

published_head=$(remote_main) ||
  blocked "POSTCHECK_UNAVAILABLE" "canonical tap main could not be read after push"
[ "$published_head" = "$prepared_head" ] ||
  blocked "POSTCHECK_HEAD_MISMATCH" "canonical tap main does not equal the pushed commit"
git -C "$tap_root" fetch --quiet "$canonical_fetch" refs/heads/main ||
  blocked "POSTCHECK_FETCH_FAILED" "published tap head could not be fetched"
[ "$(git -C "$tap_root" rev-parse FETCH_HEAD 2>/dev/null)" = "$prepared_head" ] ||
  blocked "POSTCHECK_FETCH_MISMATCH" "fetched tap head differs from the pushed commit"
git -C "$tap_root" show "FETCH_HEAD:$formula_path" >"$post_formula" 2>/dev/null ||
  blocked "POSTCHECK_FORMULA_MISSING" "published tap formula blob is unavailable"
cmp -s "$post_formula" "$formula" ||
  blocked "POSTCHECK_FORMULA_MISMATCH" "published tap formula bytes differ from the release"
[ "$(git -C "$tap_root" rev-parse HEAD)" = "$prepared_head" ] &&
  [ -z "$(git -C "$tap_root" status --porcelain)" ] ||
  blocked "POSTCHECK_LOCAL_STATE" "local tap is not clean at the published commit"

jq -cn \
  --arg version "$version" \
  --arg previousHead "$remote_head" \
  --arg publishedHead "$prepared_head" '{
    schemaVersion: 1,
    verdict: "PUBLISHED",
    version: $version,
    previousHead: $previousHead,
    publishedHead: $publishedHead
  }'

#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <tag> <source-commit> [remote]" >&2
  exit 64
}

fail() {
  echo "check-remote-release-tag: $1" >&2
  exit 1
}

[ "$#" -ge 2 ] && [ "$#" -le 3 ] || usage

tag=$1
source_commit=$2
remote=${3:-origin}

if ! printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "tag must use vX.Y.Z"
fi
if ! printf '%s\n' "$source_commit" | grep -Eq '^[0-9a-f]{40}$'; then
  fail "source commit must be a full Git SHA"
fi

remote_refs=$(git ls-remote --exit-code "$remote" \
  "refs/tags/$tag" "refs/tags/$tag^{}") ||
  fail "remote annotated tag could not be resolved: $remote $tag"

tag_ref_count=$(printf '%s\n' "$remote_refs" |
  awk -v ref="refs/tags/$tag" '$2 == ref { count += 1 } END { print count + 0 }')
peeled_ref_count=$(printf '%s\n' "$remote_refs" |
  awk -v ref="refs/tags/$tag^{}" '$2 == ref { count += 1 } END { print count + 0 }')
[ "$tag_ref_count" -eq 1 ] || fail "remote tag ref is missing or ambiguous"
[ "$peeled_ref_count" -eq 1 ] || fail "remote tag is not annotated or is ambiguous"

remote_commit=$(printf '%s\n' "$remote_refs" |
  awk -v ref="refs/tags/$tag^{}" '$2 == ref { print $1 }')
[ "$remote_commit" = "$source_commit" ] ||
  fail "remote tag resolves to $remote_commit instead of $source_commit"

printf 'REMOTE_RELEASE_TAG_OK remote=%s tag=%s source_commit=%s\n' \
  "$remote" "$tag" "$source_commit"

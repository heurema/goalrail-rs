#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <tag> [remote]" >&2
  exit 64
}

fail() {
  echo "check-remote-release-tag-absent: $1" >&2
  exit 1
}

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
  usage
fi

tag=$1
remote=${2:-origin}

if ! printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "tag must use vX.Y.Z"
fi

remote_refs=$(git ls-remote "$remote" \
  "refs/tags/$tag" "refs/tags/$tag^{}") ||
  fail "remote tags could not be queried: $remote"

[ -z "$remote_refs" ] || fail "remote release tag already exists: $remote $tag"

printf 'REMOTE_RELEASE_TAG_ABSENT remote=%s tag=%s\n' "$remote" "$tag"

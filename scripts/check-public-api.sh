#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
generator="$repo_root/scripts/generate-public-api.sh"
snapshot=${GOALRAIL_PUBLIC_API_SNAPSHOT:-"$repo_root/architecture/public-api/gr-inspect-codex.txt"}

if [ ! -f "$snapshot" ]; then
  echo "public API snapshot is missing: $snapshot" >&2
  exit 2
fi

actual=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api.XXXXXX")
trap 'rm -f "$actual"' EXIT HUP INT TERM

"$generator" >"$actual"

if ! cmp -s "$snapshot" "$actual"; then
  echo "rustdoc-visible public API differs from the accepted snapshot" >&2
  diff -u "$snapshot" "$actual" >&2 || true
  exit 1
fi

item_count=$(wc -l <"$actual" | tr -d ' ')
echo "PUBLIC_API_BASELINE_OK package=${GOALRAIL_PUBLIC_API_PACKAGE:-gr-inspect-codex} items=$item_count tool=0.52.0 target=aarch64-apple-darwin"

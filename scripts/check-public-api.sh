#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
generator="$repo_root/scripts/generate-public-api.sh"

# Every crate whose rustdoc-visible facade is pinned. A facade that is not
# listed here is not checked by this trial, so adding an inspection library
# means adding it to this list and accepting its snapshot.
pinned_packages='gr-inspect-codex gr-inspect-claude'

actual=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api.XXXXXX")
trap 'rm -f "$actual"' EXIT HUP INT TERM

check_package() {
  package=$1
  snapshot=$2

  if [ ! -f "$snapshot" ]; then
    echo "public API snapshot is missing: $snapshot" >&2
    exit 2
  fi

  GOALRAIL_PUBLIC_API_PACKAGE="$package" "$generator" >"$actual"

  if ! cmp -s "$snapshot" "$actual"; then
    echo "rustdoc-visible public API differs from the accepted snapshot for $package" >&2
    diff -u "$snapshot" "$actual" >&2 || true
    exit 1
  fi

  item_count=$(wc -l <"$actual" | tr -d ' ')
  echo "PUBLIC_API_BASELINE_OK package=$package items=$item_count tool=0.52.0 target=aarch64-apple-darwin"
}

# An explicit package or snapshot selects single-package mode, which the trial's
# own sabotage test uses to drive the checker against a fixture.
if [ -n "${GOALRAIL_PUBLIC_API_PACKAGE:-}" ] || [ -n "${GOALRAIL_PUBLIC_API_SNAPSHOT:-}" ]; then
  package=${GOALRAIL_PUBLIC_API_PACKAGE:-gr-inspect-codex}
  check_package \
    "$package" \
    "${GOALRAIL_PUBLIC_API_SNAPSHOT:-"$repo_root/architecture/public-api/$package.txt"}"
  exit 0
fi

for package in $pinned_packages; do
  check_package "$package" "$repo_root/architecture/public-api/$package.txt"
done

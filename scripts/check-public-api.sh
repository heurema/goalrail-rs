#!/bin/sh

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
generator="$repo_root/scripts/generate-public-api.sh"
snapshot_dir=${GOALRAIL_PUBLIC_API_SNAPSHOT_DIR:-"$repo_root/architecture/public-api"}
package_list=${GOALRAIL_PUBLIC_API_PACKAGE_LIST:-"$repo_root/architecture/public-api/pinned-packages.txt"}

if [ "${GOALRAIL_PUBLIC_API_GATE_TESTING:-0}" != 1 ] &&
  { [ -n "${GOALRAIL_PUBLIC_API_SNAPSHOT_DIR:-}" ] ||
    [ -n "${GOALRAIL_PUBLIC_API_PACKAGE_LIST:-}" ]; }; then
  echo "public API gate: test overrides require GOALRAIL_PUBLIC_API_GATE_TESTING=1" >&2
  exit 2
fi

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
    "${GOALRAIL_PUBLIC_API_SNAPSHOT:-"$snapshot_dir/$package.txt"}"
  exit 0
fi

# Otherwise every pinned facade is checked. The list is the single source of
# truth for which facades are pinned; `scripts/architecture-drift.sh` reads the
# same file, so the two trials cannot disagree about the pinned surface.
[ -f "$package_list" ] || {
  echo "pinned package list is missing: $package_list" >&2
  exit 2
}

checked=0
while IFS= read -r package || [ -n "$package" ]; do
  case "$package" in
    ''|\#*) continue ;;
  esac
  check_package "$package" "$snapshot_dir/$package.txt"
  checked=$((checked + 1))
done <"$package_list"

[ "$checked" -gt 0 ] || {
  echo "pinned package list is empty: $package_list" >&2
  exit 2
}

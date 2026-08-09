#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version>" >&2
  exit 64
}

[ "$#" -eq 1 ] || usage
version=$1

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  usage
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
cd "$repo_root"

findings_file=$(mktemp "${TMPDIR:-/tmp}/goalrail-preflight.XXXXXX")
cleanup() {
  rm -f "$findings_file"
}
trap cleanup EXIT HUP INT TERM

add_finding() {
  printf '%s|%s\n' "$1" "$2" >>"$findings_file"
}

[ "$(uname -s)" = "Darwin" ] ||
  add_finding "UNSUPPORTED_OS" "v0 releases must be built on macOS"
[ "$(uname -m)" = "arm64" ] ||
  add_finding "UNSUPPORTED_ARCH" "v0 releases must be built on arm64"

if ! command -v cargo >/dev/null 2>&1; then
  add_finding "CARGO_MISSING" "cargo is required to resolve the release version"
elif package_id=$(cargo pkgid -p gr 2>/dev/null); then
  crate_version=${package_id##*@}
  [ "$crate_version" = "$version" ] ||
    add_finding "VERSION_MISMATCH" "requested version does not match the gr crate"
else
  add_finding "PACKAGE_UNRESOLVED" "the gr package version could not be resolved"
fi

if [ ! -f LICENSE ] && [ ! -f LICENSE.md ]; then
  add_finding "LICENSE_MISSING" "distribution terms must be selected before publication"
fi

if ! grep -Eq '^[[:space:]]+license "[^"]+"' release/homebrew/goalrail.rb.in; then
  add_finding "FORMULA_LICENSE_MISSING" "the Homebrew formula must declare the selected license"
fi

if ! git diff --quiet || ! git diff --cached --quiet ||
  [ -n "$(git ls-files --others --exclude-standard)" ]; then
  add_finding "WORKTREE_DIRTY" "the release source must be a clean checkout"
fi

tag="v$version"
if ! git tag --points-at HEAD | grep -Fxq "$tag"; then
  add_finding "TAG_MISSING" "HEAD must have the exact immutable version tag"
fi

if [ -s "$findings_file" ]; then
  printf '{"schemaVersion":1,"verdict":"BLOCKED","version":"%s","findings":[' "$version"
  first=1
  while IFS='|' read -r code message; do
    if [ "$first" -eq 0 ]; then
      printf ','
    fi
    first=0
    printf '{"code":"%s","message":"%s"}' "$code" "$message"
  done <"$findings_file"
  printf ']}\n'
  exit 4
fi

printf '{"schemaVersion":1,"verdict":"READY","version":"%s","findings":[]}\n' "$version"

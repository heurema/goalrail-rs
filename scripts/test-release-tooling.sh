#!/bin/sh

set -eu

command -v jq >/dev/null 2>&1 || {
  echo "jq is unavailable" >&2
  exit 2
}

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
cd "$repo_root"

for script in scripts/prepare-release.sh scripts/check-release.sh scripts/release-preflight.sh; do
  sh -n "$script"
done

package_id=$(cargo pkgid -p gr)
version=${package_id##*@}
output_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-release-test.XXXXXX")

cleanup() {
  rm -rf "$output_root"
}
trap cleanup EXIT HUP INT TERM

scripts/prepare-release.sh "$version" "$output_root" >/dev/null
scripts/check-release.sh "$version" "$output_root"

if scripts/prepare-release.sh "$version-dev" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted a non-stable version" >&2
  exit 1
fi

formula="$output_root/v$version/goalrail.rb"
formula_clean="$formula.clean"
cp "$formula" "$formula_clean"
awk -v version="$version" '
  { print }
  /^[[:space:]]+sha256 / { printf "  version(\"%s\")\n", version }
' "$formula_clean" >"$formula"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted an explicit formula version" >&2
  exit 1
fi
mv "$formula_clean" "$formula"

manifest="$output_root/v$version/release.json"
manifest_clean="$manifest.clean"
cp "$manifest" "$manifest_clean"
sed 's/"product": "goalrail"/"product": "wrong"/' "$manifest_clean" >"$manifest"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted a mismatched manifest" >&2
  exit 1
fi
cp "$manifest_clean" "$manifest"

jq '. + {unexpected: true}' "$manifest_clean" >"$manifest"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted an extra manifest field" >&2
  exit 1
fi

jq '.schemaVersion = "1"' "$manifest_clean" >"$manifest"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted a mistyped manifest field" >&2
  exit 1
fi
mv "$manifest_clean" "$manifest"

artifact="$output_root/v$version/goalrail-v$version-aarch64-apple-darwin.tar.gz"
printf 'tampered' >>"$artifact"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted a tampered archive" >&2
  exit 1
fi

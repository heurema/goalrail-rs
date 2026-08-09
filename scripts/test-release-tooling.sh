#!/bin/sh

set -eu

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

artifact="$output_root/v$version/goalrail-v$version-aarch64-apple-darwin.tar.gz"
printf 'tampered' >>"$artifact"
if scripts/check-release.sh "$version" "$output_root" >/dev/null 2>&1; then
  echo "release tooling accepted a tampered archive" >&2
  exit 1
fi

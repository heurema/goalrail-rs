#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <source-commit> [output-root]" >&2
  exit 64
}

fail() {
  echo "check-release-bundle: $1" >&2
  exit 1
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
    fail "no SHA-256 command is available"
  fi
}

[ "$#" -ge 2 ] && [ "$#" -le 3 ] || usage

version=$1
source_commit=$2
output_root=${3:-dist}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi
if ! printf '%s\n' "$source_commit" | grep -Eq '^[0-9a-f]{40}$'; then
  fail "source commit must be a full Git SHA"
fi

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
command -v ruby >/dev/null 2>&1 || fail "ruby is unavailable for Homebrew formula validation"

tag="v$version"
release_dir="$output_root/$tag"
formula="$release_dir/goalrail.rb"
release_manifest="$release_dir/release.json"

for file in "$formula" "$release_manifest"; do
  [ -f "$file" ] || fail "required bundle file is missing: $file"
done

targets='aarch64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-msvc'
manifests=
for target in $targets; do
  "$script_dir/check-release.sh" "$version" "$target" "$output_root" >/dev/null
  manifest="$release_dir/goalrail-$tag-$target.json"
  [ "$(jq -er '.build.sourceCommit' "$manifest")" = "$source_commit" ] ||
    fail "$target source commit does not match"
  manifests="$manifests $manifest"
done

lock_sha256=$(checksum Cargo.lock)
expected_manifest=$(mktemp "${TMPDIR:-/tmp}/goalrail-release-manifest.XXXXXX")
actual_sorted=$(mktemp "${TMPDIR:-/tmp}/goalrail-release-actual.XXXXXX")
expected_sorted=$(mktemp "${TMPDIR:-/tmp}/goalrail-release-expected.XXXXXX")
cleanup() {
  rm -f "$expected_manifest" "$actual_sorted" "$expected_sorted"
}
trap cleanup EXIT HUP INT TERM

# shellcheck disable=SC2086
jq -s \
  --arg version "$version" \
  --arg tag "$tag" \
  --arg source_commit "$source_commit" \
  --arg lock_sha256 "$lock_sha256" \
  '{
    schemaVersion: 2,
    product: "goalrail",
    version: $version,
    tag: $tag,
    sourceCommit: $source_commit,
    cargoLockSha256: $lock_sha256,
    license: "MIT",
    artifacts: (sort_by(.target))
  }' $manifests >"$expected_manifest"

jq -S . "$release_manifest" >"$actual_sorted"
jq -S . "$expected_manifest" >"$expected_sorted"
cmp -s "$actual_sorted" "$expected_sorted" ||
  fail "aggregate release manifest does not match target artifacts"

mac_manifest="$release_dir/goalrail-$tag-aarch64-apple-darwin.json"
mac_url=$(jq -er '.downloadUrl' "$mac_manifest")
mac_sha256=$(jq -er '.sha256' "$mac_manifest")

if grep -q '@@' "$formula"; then
  fail "Homebrew formula contains unresolved placeholders"
fi
grep -Fq "url \"$mac_url\"" "$formula" || fail "formula URL does not match"
grep -Fq "sha256 \"$mac_sha256\"" "$formula" || fail "formula checksum does not match"
grep -Fq 'depends_on :macos' "$formula" || fail "formula lacks the macOS guard"
grep -Fq 'depends_on arch: :arm64' "$formula" || fail "formula lacks the arm64 guard"
if grep -Eq '^[[:space:]]+version([[:space:]]|\()' "$formula"; then
  fail "formula should derive its version from the release URL"
fi

ruby -c "$formula" >/dev/null || fail "Homebrew formula has invalid Ruby syntax"

printf '{"schemaVersion":2,"verdict":"READY","version":"%s","sourceCommit":"%s","targets":3}\n' \
  "$version" "$source_commit"

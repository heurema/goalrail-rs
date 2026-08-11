#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <source-commit> [output-root]" >&2
  exit 64
}

fail() {
  echo "assemble-release: $1" >&2
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
[ -f LICENSE ] || fail "LICENSE is missing"

tag="v$version"
release_dir="$output_root/$tag"
formula="$release_dir/goalrail.rb"
release_manifest="$release_dir/release.json"

[ ! -e "$formula" ] || fail "output already exists: $formula"
[ ! -e "$release_manifest" ] || fail "output already exists: $release_manifest"

targets='aarch64-apple-darwin x86_64-unknown-linux-gnu x86_64-pc-windows-msvc'
manifests=
for target in $targets; do
  "$script_dir/check-release.sh" "$version" "$target" "$output_root" >/dev/null
  manifest="$release_dir/goalrail-$tag-$target.json"
  manifest_commit=$(jq -er '.build.sourceCommit' "$manifest")
  [ "$manifest_commit" = "$source_commit" ] ||
    fail "$target was built from $manifest_commit instead of $source_commit"
  manifests="$manifests $manifest"
done

lock_sha256=$(checksum Cargo.lock)
for manifest in $manifests; do
  manifest_lock_sha256=$(jq -er '.build.cargoLockSha256' "$manifest")
  [ "$manifest_lock_sha256" = "$lock_sha256" ] ||
    fail "artifact lock digest does not match Cargo.lock: $manifest"
done

mac_manifest="$release_dir/goalrail-$tag-aarch64-apple-darwin.json"
mac_url=$(jq -er '.downloadUrl' "$mac_manifest")
mac_sha256=$(jq -er '.sha256' "$mac_manifest")

sed \
  -e "s|@@URL@@|$mac_url|g" \
  -e "s|@@SHA256@@|$mac_sha256|g" \
  release/homebrew/goalrail.rb.in >"$formula"

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
  }' $manifests >"$release_manifest"

"$script_dir/check-release-bundle.sh" \
  "$version" "$source_commit" "$output_root" >/dev/null
printf '%s\n' "$release_dir"

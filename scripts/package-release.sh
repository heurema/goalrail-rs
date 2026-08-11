#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <target> <binary> [output-root]" >&2
  exit 64
}

fail() {
  echo "package-release: $1" >&2
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

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  usage
fi

version=$1
target=$2
binary=$3
output_root=${4:-dist}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi

case "$target" in
  aarch64-apple-darwin|x86_64-unknown-linux-gnu) binary_name=gr ;;
  x86_64-pc-windows-msvc) binary_name=gr.exe ;;
  *) fail "unsupported release target: $target" ;;
esac

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v cargo >/dev/null 2>&1 || fail "cargo is unavailable"
command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
command -v rustc >/dev/null 2>&1 || fail "rustc is unavailable"
command -v tar >/dev/null 2>&1 || fail "tar is unavailable"
[ -f LICENSE ] || fail "LICENSE is missing"
[ -f "$binary" ] || fail "release binary is missing: $binary"

tag="v$version"
artifact="goalrail-$tag-$target.tar.gz"
manifest="goalrail-$tag-$target.json"
download_url="https://github.com/heurema/goalrail-rs/releases/download/$tag/$artifact"
release_dir="$output_root/$tag"

for path in \
  "$release_dir/$artifact" \
  "$release_dir/$artifact.sha256" \
  "$release_dir/$manifest"; do
  [ ! -e "$path" ] || fail "output already exists: $path"
done

mkdir -p "$output_root" "$release_dir"
bundle_dir=$(mktemp -d "$output_root/.goalrail-$tag-$target.XXXXXX")
payload_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-payload.XXXXXX")

cleanup() {
  rm -rf "$bundle_dir" "$payload_dir"
}
trap cleanup EXIT HUP INT TERM

cp "$binary" "$payload_dir/$binary_name"
chmod 0755 "$payload_dir/$binary_name"
cp LICENSE "$payload_dir/LICENSE"
chmod 0644 "$payload_dir/LICENSE"
COPYFILE_DISABLE=1 tar -C "$payload_dir" -czf \
  "$bundle_dir/$artifact" LICENSE "$binary_name"

archive_sha256=$(checksum "$bundle_dir/$artifact")
archive_size=$(wc -c <"$bundle_dir/$artifact" | tr -d ' ')
printf '%s  %s\n' "$archive_sha256" "$artifact" >"$bundle_dir/$artifact.sha256"

source_commit=$(git rev-parse --verify HEAD 2>/dev/null) ||
  fail "cannot resolve the source commit"
lock_sha256=$(checksum Cargo.lock)
rustc_release=$(rustc -vV | awk '/^release:/ { print $2 }')
rustc_host=$(rustc -vV | awk '/^host:/ { print $2 }')
runner_image=${ImageOS:-${RUNNER_OS:-local}}
runner_image_version=${ImageVersion:-unknown}

jq -n \
  --arg version "$version" \
  --arg tag "$tag" \
  --arg target "$target" \
  --arg binary "$binary_name" \
  --arg binary_version "gr $version" \
  --arg artifact "$artifact" \
  --arg sha256 "$archive_sha256" \
  --arg checksum_artifact "$artifact.sha256" \
  --arg download_url "$download_url" \
  --arg source_commit "$source_commit" \
  --arg lock_sha256 "$lock_sha256" \
  --arg rustc_release "$rustc_release" \
  --arg rustc_host "$rustc_host" \
  --arg runner_image "$runner_image" \
  --arg runner_image_version "$runner_image_version" \
  --argjson artifact_size "$archive_size" \
  '{
    schemaVersion: 1,
    product: "goalrail",
    version: $version,
    tag: $tag,
    target: $target,
    binary: $binary,
    binaryVersion: $binary_version,
    license: "MIT",
    artifact: $artifact,
    artifactSizeBytes: $artifact_size,
    sha256: $sha256,
    checksumArtifact: $checksum_artifact,
    downloadUrl: $download_url,
    build: {
      sourceCommit: $source_commit,
      cargoLockSha256: $lock_sha256,
      rustcRelease: $rustc_release,
      rustcHost: $rustc_host,
      runnerImage: $runner_image,
      runnerImageVersion: $runner_image_version
    }
  }' >"$bundle_dir/$manifest"

mv "$bundle_dir/$artifact" "$release_dir/$artifact"
mv "$bundle_dir/$artifact.sha256" "$release_dir/$artifact.sha256"
mv "$bundle_dir/$manifest" "$release_dir/$manifest"

printf '%s\n' "$release_dir"

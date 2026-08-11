#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <target> [output-root]" >&2
  exit 64
}

fail() {
  echo "check-release: $1" >&2
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
target=$2
output_root=${3:-dist}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi

case "$target" in
  aarch64-apple-darwin|x86_64-unknown-linux-gnu) binary_name=gr ;;
  x86_64-pc-windows-msvc) binary_name=gr.exe ;;
  *) fail "unsupported release target: $target" ;;
esac

tag="v$version"
artifact="goalrail-$tag-$target.tar.gz"
release_dir="$output_root/$tag"
archive="$release_dir/$artifact"
checksum_file="$archive.sha256"
manifest="$release_dir/goalrail-$tag-$target.json"
download_url="https://github.com/heurema/goalrail-rs/releases/download/$tag/$artifact"

for file in "$archive" "$checksum_file" "$manifest"; do
  [ -f "$file" ] || fail "required artifact is missing: $file"
done

recorded_checksum=$(awk 'NR == 1 { print $1 }' "$checksum_file")
recorded_name=$(awk 'NR == 1 { print $2 }' "$checksum_file")
[ "$recorded_name" = "$artifact" ] || fail "checksum filename does not match"

actual_checksum=$(checksum "$archive")
[ "$actual_checksum" = "$recorded_checksum" ] || fail "archive checksum does not match"

archive_entries=$(tar -tzf "$archive")
[ "$archive_entries" = "LICENSE
$binary_name" ] || fail "archive must contain only LICENSE and $binary_name"

extract_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-check.XXXXXX")
cleanup() {
  rm -rf "$extract_dir"
}
trap cleanup EXIT HUP INT TERM

tar -C "$extract_dir" -xzf "$archive"
[ -s "$extract_dir/$binary_name" ] || fail "archived binary is empty"
cmp -s LICENSE "$extract_dir/LICENSE" || fail "archived license does not match"
if [ "$target" != x86_64-pc-windows-msvc ]; then
  [ -x "$extract_dir/$binary_name" ] || fail "archived binary is not executable"
fi

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
command -v file >/dev/null 2>&1 || fail "file is unavailable"
archive_size=$(wc -c <"$archive" | tr -d ' ')
binary_description=$(LC_ALL=C file -b "$extract_dir/$binary_name")
case "$target:$binary_description" in
  aarch64-apple-darwin:*'Mach-O 64-bit executable arm64'*) ;;
  x86_64-unknown-linux-gnu:*'ELF 64-bit'*'x86-64'*) ;;
  x86_64-pc-windows-msvc:*'PE32+ executable'*'x86-64'*) ;;
  *) fail "binary format does not match $target: $binary_description" ;;
esac

jq -e \
  --arg version "$version" \
  --arg tag "$tag" \
  --arg target "$target" \
  --arg binary "$binary_name" \
  --arg artifact "$artifact" \
  --arg sha256 "$actual_checksum" \
  --arg download_url "$download_url" \
  --argjson artifact_size "$archive_size" \
  '.schemaVersion == 1
  and .product == "goalrail"
  and .version == $version
  and .tag == $tag
  and .target == $target
  and .binary == $binary
  and .binaryVersion == ("gr " + $version)
  and .license == "MIT"
  and .artifact == $artifact
  and .artifactSizeBytes == $artifact_size
  and .sha256 == $sha256
  and .checksumArtifact == ($artifact + ".sha256")
  and .downloadUrl == $download_url
  and (.build.sourceCommit | test("^[0-9a-f]{40}$"))
  and (.build.cargoLockSha256 | test("^[0-9a-f]{64}$"))
  and (.build.rustcRelease | test("^[0-9]+\\.[0-9]+\\.[0-9]+$"))
  and .build.rustcHost == $target
  and (.build.runnerImage | type == "string" and length > 0)
  and (.build.runnerImageVersion | type == "string" and length > 0)
  and (keys | sort) == ([
    "artifact", "artifactSizeBytes", "binary", "binaryVersion", "build",
    "checksumArtifact", "downloadUrl", "license", "product",
    "schemaVersion", "sha256", "tag", "target", "version"
  ] | sort)
  and (.build | keys | sort) == ([
    "cargoLockSha256", "runnerImage", "runnerImageVersion", "rustcHost",
    "rustcRelease", "sourceCommit"
  ] | sort)' "$manifest" >/dev/null || fail "release manifest does not match artifact"

printf '{"schemaVersion":1,"verdict":"READY","version":"%s","target":"%s","artifact":"%s","sha256":"%s"}\n' \
  "$version" "$target" "$artifact" "$actual_checksum"

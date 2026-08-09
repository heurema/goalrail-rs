#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> [output-root]" >&2
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

[ "$#" -ge 1 ] && [ "$#" -le 2 ] || usage

version=$1
output_root=${2:-dist}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi

target=aarch64-apple-darwin
tag="v$version"
artifact="goalrail-$tag-$target.tar.gz"
release_dir="$output_root/$tag"
archive="$release_dir/$artifact"
checksum_file="$archive.sha256"
formula="$release_dir/goalrail.rb"
manifest="$release_dir/release.json"
download_url="https://github.com/heurema/goalrail-rs/releases/download/$tag/$artifact"

for file in "$archive" "$checksum_file" "$formula" "$manifest"; do
  [ -f "$file" ] || fail "required artifact is missing: $file"
done

recorded_checksum=$(awk 'NR == 1 { print $1 }' "$checksum_file")
recorded_name=$(awk 'NR == 1 { print $2 }' "$checksum_file")
[ "$recorded_name" = "$artifact" ] || fail "checksum filename does not match"

actual_checksum=$(checksum "$archive")
[ "$actual_checksum" = "$recorded_checksum" ] || fail "archive checksum does not match"

archive_entries=$(tar -tzf "$archive")
[ "$archive_entries" = "LICENSE
gr" ] || fail "archive must contain only LICENSE and gr"

extract_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-check.XXXXXX")
cleanup() {
  rm -rf "$extract_dir"
}
trap cleanup EXIT HUP INT TERM

tar -C "$extract_dir" -xzf "$archive"
[ -x "$extract_dir/gr" ] || fail "archived gr is not executable"
cmp -s LICENSE "$extract_dir/LICENSE" || fail "archived license does not match"
[ "$("$extract_dir/gr" --version)" = "gr $version" ] ||
  fail "archived gr reports an unexpected version"

if command -v lipo >/dev/null 2>&1; then
  lipo "$extract_dir/gr" -verify_arch arm64 >/dev/null 2>&1 ||
    fail "archived gr is not arm64"
fi

command -v ruby >/dev/null 2>&1 || fail "ruby is unavailable"
ruby -c "$formula" >/dev/null || fail "Homebrew formula has invalid Ruby syntax"

if grep -q '@@' "$formula"; then
  fail "Homebrew formula contains unresolved placeholders"
fi

grep -Fq "version \"$version\"" "$formula" || fail "formula version does not match"
grep -Fq "url \"$download_url\"" "$formula" || fail "formula URL does not match"
grep -Fq "sha256 \"$actual_checksum\"" "$formula" || fail "formula checksum does not match"
grep -Fq 'depends_on :macos' "$formula" || fail "formula lacks the macOS guard"
grep -Fq 'depends_on arch: :arm64' "$formula" || fail "formula lacks the arm64 guard"

ruby -rjson -e '
  manifest = JSON.parse(File.read(ARGV.fetch(0)))
  expected = {
    "schemaVersion" => 1,
    "product" => "goalrail",
    "version" => ARGV.fetch(1),
    "tag" => ARGV.fetch(2),
    "target" => ARGV.fetch(3),
    "binary" => "gr",
    "binaryVersion" => "gr #{ARGV.fetch(1)}",
    "license" => "MIT",
    "artifact" => ARGV.fetch(4),
    "artifactSizeBytes" => File.size(ARGV.fetch(7)),
    "sha256" => ARGV.fetch(5),
    "checksumArtifact" => "#{ARGV.fetch(4)}.sha256",
    "downloadUrl" => ARGV.fetch(6)
  }
  abort "release manifest does not match artifacts" unless manifest == expected
' "$manifest" "$version" "$tag" "$target" "$artifact" "$actual_checksum" "$download_url" "$archive"

printf '{"schemaVersion":1,"verdict":"READY","version":"%s","artifact":"%s","sha256":"%s"}\n' \
  "$version" "$artifact" "$actual_checksum"

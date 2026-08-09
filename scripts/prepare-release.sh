#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> [output-root]" >&2
  exit 64
}

fail() {
  echo "prepare-release: $1" >&2
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

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
cd "$repo_root"

[ "$(uname -s)" = "Darwin" ] || fail "v0 release packaging requires macOS"
[ "$(uname -m)" = "arm64" ] || fail "v0 release packaging requires arm64"
command -v cargo >/dev/null 2>&1 || fail "cargo is unavailable"
command -v tar >/dev/null 2>&1 || fail "tar is unavailable"
[ -f LICENSE ] || fail "LICENSE is missing"

package_id=$(cargo pkgid -p gr 2>/dev/null) || fail "cannot resolve the gr package"
crate_version=${package_id##*@}
[ "$crate_version" = "$version" ] ||
  fail "requested version $version does not match gr crate version $crate_version"

target=aarch64-apple-darwin
tag="v$version"
artifact="goalrail-$tag-$target.tar.gz"
download_url="https://github.com/heurema/goalrail-rs/releases/download/$tag/$artifact"
release_dir="$output_root/$tag"

[ ! -e "$release_dir" ] || fail "output already exists: $release_dir"

mkdir -p "$output_root"
bundle_dir=$(mktemp -d "$output_root/.goalrail-$tag.XXXXXX")
payload_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-payload.XXXXXX")

cleanup() {
  rm -rf "$bundle_dir" "$payload_dir"
}
trap cleanup EXIT HUP INT TERM

cargo build --release --locked --target "$target" -p gr

binary="target/$target/release/gr"
[ -x "$binary" ] || fail "release binary is missing: $binary"
[ "$("$binary" --version)" = "gr $version" ] ||
  fail "release binary reports an unexpected version"

if command -v lipo >/dev/null 2>&1; then
  lipo "$binary" -verify_arch arm64 >/dev/null 2>&1 ||
    fail "release binary is not arm64"
fi

install -m 0755 "$binary" "$payload_dir/gr"
install -m 0644 LICENSE "$payload_dir/LICENSE"
COPYFILE_DISABLE=1 tar -C "$payload_dir" -czf "$bundle_dir/$artifact" LICENSE gr

archive_sha256=$(checksum "$bundle_dir/$artifact")
archive_size=$(wc -c <"$bundle_dir/$artifact" | tr -d ' ')
printf '%s  %s\n' "$archive_sha256" "$artifact" >"$bundle_dir/$artifact.sha256"

sed \
  -e "s|@@VERSION@@|$version|g" \
  -e "s|@@URL@@|$download_url|g" \
  -e "s|@@SHA256@@|$archive_sha256|g" \
  release/homebrew/goalrail.rb.in >"$bundle_dir/goalrail.rb"

cat >"$bundle_dir/release.json" <<EOF
{
  "schemaVersion": 1,
  "product": "goalrail",
  "version": "$version",
  "tag": "$tag",
  "target": "$target",
  "binary": "gr",
  "binaryVersion": "gr $version",
  "license": "MIT",
  "artifact": "$artifact",
  "artifactSizeBytes": $archive_size,
  "sha256": "$archive_sha256",
  "checksumArtifact": "$artifact.sha256",
  "downloadUrl": "$download_url"
}
EOF

mv "$bundle_dir" "$release_dir"
trap - EXIT HUP INT TERM
rm -rf "$payload_dir"

printf '%s\n' "$release_dir"

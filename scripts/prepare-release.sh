#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <target> [output-root]" >&2
  exit 64
}

fail() {
  echo "prepare-release: $1" >&2
  exit 1
}

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  usage
fi

version=$1
target=$2
output_root=${3:-dist}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi

case "$target" in
  aarch64-apple-darwin) binary_name=gr ;;
  x86_64-unknown-linux-gnu) binary_name=gr ;;
  x86_64-pc-windows-msvc) binary_name=gr.exe ;;
  *) fail "unsupported release target: $target" ;;
esac

system=$(uname -s)
machine=$(uname -m)
case "$system:$machine" in
  Darwin:arm64) host_target=aarch64-apple-darwin ;;
  Linux:x86_64) host_target=x86_64-unknown-linux-gnu ;;
  MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64)
    host_target=x86_64-pc-windows-msvc
    ;;
  *) fail "unsupported native release host: $system $machine" ;;
esac

[ "$target" = "$host_target" ] ||
  fail "target $target must be built on its native host $host_target"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v cargo >/dev/null 2>&1 || fail "cargo is unavailable"
[ -f LICENSE ] || fail "LICENSE is missing"

package_id=$(cargo pkgid -p gr 2>/dev/null) || fail "cannot resolve the gr package"
crate_version=${package_id##*@}
[ "$crate_version" = "$version" ] ||
  fail "requested version $version does not match gr crate version $crate_version"

cargo build --release --locked --target "$target" -p gr

binary="target/$target/release/$binary_name"
[ -f "$binary" ] || fail "release binary is missing: $binary"
[ "$("$binary" --version 2>/dev/null)" = "gr $version" ] ||
  fail "release binary reports an unexpected version"

"$script_dir/smoke-release-binary.sh" "$version" "$binary"
"$script_dir/package-release.sh" "$version" "$target" "$binary" "$output_root"

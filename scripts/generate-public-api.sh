#!/bin/sh

set -eu

fail() {
  echo "public-api-generator: $1" >&2
  exit 2
}

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tool=${GOALRAIL_CARGO_PUBLIC_API:-cargo-public-api}
manifest=${GOALRAIL_PUBLIC_API_MANIFEST:-"$repo_root/Cargo.toml"}
package=${GOALRAIL_PUBLIC_API_PACKAGE:-gr-inspect-codex}
expected_tool_version='cargo-public-api 0.52.0'
nightly_toolchain='nightly-2026-08-07'
expected_nightly='rustc 1.99.0-nightly (84b36a78a 2026-08-06)'
target='aarch64-apple-darwin'

command -v "$tool" >/dev/null 2>&1 ||
  fail "cargo-public-api is required; run mise run setup"

actual_tool_version=$("$tool" --version)
if [ "$actual_tool_version" != "$expected_tool_version" ]; then
  fail "cargo-public-api version mismatch: expected '$expected_tool_version', got '$actual_tool_version'"
fi

actual_nightly=$(rustc +"$nightly_toolchain" --version)
if [ "$actual_nightly" != "$expected_nightly" ]; then
  fail "nightly version mismatch: expected '$expected_nightly', got '$actual_nightly'"
fi

work_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-public-api-generate.XXXXXX")

cleanup() {
  rm -rf "$work_root"
}
trap cleanup EXIT HUP INT TERM

actual="$work_root/actual.txt"
diagnostics="$work_root/diagnostics.txt"
errors="$work_root/errors.txt"
debug_errors="$work_root/debug-errors.txt"

run_public_api() {
  rustup run "$nightly_toolchain" "$tool" \
    --manifest-path "$manifest" \
    --package "$package" \
    --target "$target" \
    --color never \
    --no-default-features \
    --omit blanket-impls,auto-trait-impls,auto-derived-impls \
    "$@"
}

if ! run_public_api >"$actual" 2>"$errors"; then
  echo "cargo-public-api failed while generating the facade snapshot" >&2
  cat "$errors" >&2
  exit 1
fi

if ! run_public_api --debug-processing >"$diagnostics" 2>"$debug_errors"; then
  echo "cargo-public-api failed while checking rustdoc completeness" >&2
  cat "$debug_errors" >&2
  exit 1
fi

if grep -Fq 'NOTE: rustdoc JSON missing referenced item' "$diagnostics"; then
  echo "cargo-public-api produced unresolved rustdoc references" >&2
  grep -F 'NOTE: rustdoc JSON missing referenced item' "$diagnostics" >&2
  exit 1
fi

cat "$actual"

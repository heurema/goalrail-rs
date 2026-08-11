#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
tool=${GOALRAIL_CARGO_PUBLIC_API:-cargo-public-api}
manifest=${GOALRAIL_PUBLIC_API_MANIFEST:-"$repo_root/Cargo.toml"}
package=${GOALRAIL_PUBLIC_API_PACKAGE:-gr-inspect-codex}
snapshot=${GOALRAIL_PUBLIC_API_SNAPSHOT:-"$repo_root/architecture/public-api/gr-inspect-codex.txt"}
expected_tool_version='cargo-public-api 0.52.0'
nightly_toolchain='nightly-2026-08-07'
expected_nightly='rustc 1.99.0-nightly (84b36a78a 2026-08-06)'
target='aarch64-apple-darwin'

if ! command -v "$tool" >/dev/null 2>&1; then
  echo "cargo-public-api is required; run mise run setup" >&2
  exit 2
fi

actual_tool_version=$("$tool" --version)
if [ "$actual_tool_version" != "$expected_tool_version" ]; then
  echo "cargo-public-api version mismatch: expected '$expected_tool_version', got '$actual_tool_version'" >&2
  exit 2
fi

actual_nightly=$(rustc +"$nightly_toolchain" --version)
if [ "$actual_nightly" != "$expected_nightly" ]; then
  echo "nightly version mismatch: expected '$expected_nightly', got '$actual_nightly'" >&2
  exit 2
fi

if [ ! -f "$snapshot" ]; then
  echo "public API snapshot is missing: $snapshot" >&2
  exit 2
fi

actual=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api.XXXXXX")
diagnostics=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api-debug.XXXXXX")
errors=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api-errors.XXXXXX")
debug_errors=$(mktemp "${TMPDIR:-/tmp}/goalrail-public-api-debug-errors.XXXXXX")
trap 'rm -f "$actual" "$diagnostics" "$errors" "$debug_errors"' EXIT HUP INT TERM

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

if ! cmp -s "$snapshot" "$actual"; then
  echo "rustdoc-visible public API differs from the accepted snapshot" >&2
  diff -u "$snapshot" "$actual" >&2 || true
  exit 1
fi

item_count=$(wc -l <"$actual" | tr -d ' ')
echo "PUBLIC_API_BASELINE_OK package=$package items=$item_count tool=0.52.0 target=$target"

#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
checker="$repo_root/scripts/check-public-api.sh"
tmp_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-public-api-test.XXXXXX")
trap 'rm -rf "$tmp_root"' EXIT HUP INT TERM

snapshot="$tmp_root/snapshot.txt"
fake_tool="$tmp_root/cargo-public-api"

printf '%s\n' 'pub fn fixture::stable()' >"$snapshot"

cat >"$fake_tool" <<'EOF'
#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  if [ "${FAKE_PUBLIC_API_MODE:-}" = "old-version" ]; then
    echo 'cargo-public-api 0.51.0'
  else
    echo 'cargo-public-api 0.52.0'
  fi
  exit 0
fi

debug=false
for argument in "$@"; do
  if [ "$argument" = "--debug-processing" ]; then
    debug=true
  fi
done

if [ "${FAKE_PUBLIC_API_MODE:-}" = "unresolved" ] && [ "$debug" = true ]; then
  echo 'NOTE: rustdoc JSON missing referenced item with ID "7"'
  exit 0
fi

if [ "${FAKE_PUBLIC_API_MODE:-}" = "changed" ]; then
  echo 'pub fn fixture::changed()'
else
  echo 'pub fn fixture::stable()'
fi
EOF
chmod +x "$fake_tool"

run_checker() {
  GOALRAIL_CARGO_PUBLIC_API="$fake_tool" \
    GOALRAIL_PUBLIC_API_MANIFEST="$repo_root/Cargo.toml" \
    GOALRAIL_PUBLIC_API_PACKAGE=fixture \
    GOALRAIL_PUBLIC_API_SNAPSHOT="$snapshot" \
    FAKE_PUBLIC_API_MODE="$1" \
    "$checker"
}

run_checker pass >/dev/null

if run_checker unresolved >/dev/null 2>&1; then
  echo "expected unresolved rustdoc references to fail" >&2
  exit 1
fi

if run_checker changed >/dev/null 2>&1; then
  echo "expected a changed public API snapshot to fail" >&2
  exit 1
fi

if run_checker old-version >/dev/null 2>&1; then
  echo "expected a cargo-public-api version mismatch to fail" >&2
  exit 1
fi

echo "public API trial tests passed"

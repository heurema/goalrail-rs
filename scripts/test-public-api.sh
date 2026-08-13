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

# Multi-package mode: the loop that checks every pinned facade needs its own
# coverage, or removing a package from the list leaves this trial green while
# the facade it claims to pin goes unchecked.
multi_root="$tmp_root/multi"
mkdir -p "$multi_root"
package_list="$multi_root/pinned-packages.txt"
printf '%s\n' 'alpha-crate' '# a comment line is skipped' 'beta-crate' >"$package_list"
printf '%s\n' 'pub fn alpha::stable()' >"$multi_root/alpha-crate.txt"
printf '%s\n' 'pub fn beta::stable()' >"$multi_root/beta-crate.txt"

cat >"$fake_tool" <<'EOF'
#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  echo 'cargo-public-api 0.52.0'
  exit 0
fi

package=${GOALRAIL_PUBLIC_API_PACKAGE:-unknown}
if [ "$package" = "beta-crate" ] && [ "${FAKE_PUBLIC_API_MODE:-}" = "beta-changed" ]; then
  echo 'pub fn beta::changed()'
  exit 0
fi

case "$package" in
  alpha-crate) echo 'pub fn alpha::stable()' ;;
  beta-crate) echo 'pub fn beta::stable()' ;;
  *) echo "unexpected package: $package" >&2; exit 1 ;;
esac
EOF
chmod +x "$fake_tool"

run_multi_checker() {
  GOALRAIL_PUBLIC_API_GATE_TESTING=1 \
    GOALRAIL_CARGO_PUBLIC_API="$fake_tool" \
    GOALRAIL_PUBLIC_API_MANIFEST="$repo_root/Cargo.toml" \
    GOALRAIL_PUBLIC_API_PACKAGE_LIST="$package_list" \
    GOALRAIL_PUBLIC_API_SNAPSHOT_DIR="$multi_root" \
    FAKE_PUBLIC_API_MODE="${1:-}" \
    "$checker"
}

multi_output=$(run_multi_checker) || {
  echo "expected both pinned packages to pass" >&2
  exit 1
}
for expected in alpha-crate beta-crate; do
  printf '%s\n' "$multi_output" | grep -q "package=$expected" || {
    echo "expected multi-package mode to check $expected" >&2
    exit 1
  }
done

if run_multi_checker beta-changed >/dev/null 2>&1; then
  echo "expected a changed second-package snapshot to fail" >&2
  exit 1
fi

if GOALRAIL_PUBLIC_API_PACKAGE_LIST="$package_list" "$checker" >/dev/null 2>&1; then
  echo "expected an unguarded package list override to fail" >&2
  exit 1
fi

printf '%s\n' '# every entry commented out' >"$package_list"
if run_multi_checker >/dev/null 2>&1; then
  echo "expected an empty pinned package list to fail" >&2
  exit 1
fi

# The pinned list itself is the claim; assert it names the real facades rather
# than trusting that a fixture-driven loop implies the production one.
for expected in gr-inspect-codex gr-inspect-claude; do
  grep -qx "$expected" "$repo_root/architecture/public-api/pinned-packages.txt" || {
    echo "expected $expected to be pinned in architecture/public-api/pinned-packages.txt" >&2
    exit 1
  }
  test -f "$repo_root/architecture/public-api/$expected.txt" || {
    echo "expected an accepted snapshot for $expected" >&2
    exit 1
  }
done

echo "public API trial tests passed"

#!/bin/sh

set -eu

fail() {
  echo "homebrew-update-state-test: $1" >&2
  exit 1
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
subject="$repo_root/plugins/goalrail/skills/goalrail/scripts/homebrew-update-state.sh"

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
[ -x "$subject" ] || fail "bundled Homebrew diagnostic is not executable"
sh -n "$subject"

fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-brew-state-test.XXXXXX")
cleanup() {
  rm -rf "$fixture_root"
}
trap cleanup EXIT HUP INT TERM

fake_bin="$fixture_root/bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/brew" <<'EOF'
#!/bin/sh

set -u

scenario=${GOALRAIL_BREW_SCENARIO:?}
[ "${HOMEBREW_NO_AUTO_UPDATE:-}" = 1 ] || {
  echo "Homebrew auto-update was not disabled" >&2
  exit 94
}
operation=$1
shift

case "$operation" in
  info)
    case "$scenario" in
      info-noise)
        printf 'warning: not json\n'
        exit 0
        ;;
      info-missing-installed)
        printf '{"formulae":[{"full_name":"heurema/tap/goalrail","versions":{"stable":"1.2.3"}}],"casks":[]}\n'
        exit 0
        ;;
      not-installed)
        installed='[]'
        ;;
      no-change|exit-zero-with-entry)
        installed='[{"version":"1.2.3"}]'
        ;;
      *)
        installed='[{"version":"1.2.2"}]'
        ;;
    esac
    printf '{"formulae":[{"full_name":"heurema/tap/goalrail","versions":{"stable":"1.2.3"},"installed":%s}],"casks":[]}\n' "$installed"
    ;;
  outdated)
    case "$scenario" in
      not-installed)
        echo "outdated must not run for an absent installation" >&2
        exit 91
        ;;
      no-change)
        printf '{"formulae":[],"casks":[]}\n'
        exit 0
        ;;
      update)
        printf '{"formulae":[{"name":"goalrail","installed_versions":["1.2.2"],"current_version":"1.2.3"}],"casks":[]}\n'
        exit 1
        ;;
      unsupported-status)
        printf '{"formulae":[],"casks":[]}\n'
        exit 2
        ;;
      noise)
        printf 'warning: not json\n'
        exit 1
        ;;
      exit-zero-with-entry)
        printf '{"formulae":[{"name":"goalrail","installed_versions":["1.2.3"],"current_version":"1.2.3"}],"casks":[]}\n'
        exit 0
        ;;
      exit-one-empty)
        printf '{"formulae":[],"casks":[]}\n'
        exit 1
        ;;
      mismatched-current)
        printf '{"formulae":[{"name":"goalrail","installed_versions":["1.2.2"],"current_version":"1.2.4"}],"casks":[]}\n'
        exit 1
        ;;
      mismatched-installed)
        printf '{"formulae":[{"name":"goalrail","installed_versions":["1.2.1"],"current_version":"1.2.3"}],"casks":[]}\n'
        exit 1
        ;;
      *)
        echo "unknown scenario: $scenario" >&2
        exit 92
        ;;
    esac
    ;;
  *)
    echo "unexpected brew operation: $operation" >&2
    exit 93
    ;;
esac
EOF
chmod 0755 "$fake_bin/brew"

run_success() {
  scenario=$1
  expected=$2
  output=$(GOALRAIL_BREW_SCENARIO="$scenario" PATH="$fake_bin:$PATH" "$subject") ||
    fail "$scenario unexpectedly failed"
  printf '%s\n' "$output" | jq -e --arg verdict "$expected" '
    .schemaVersion == 1 and .verdict == $verdict and .manager == "HOMEBREW"
  ' >/dev/null || fail "$scenario returned the wrong verdict"
  printf '%s\n' "$output"
}

run_blocked() {
  scenario=$1
  expected_code=$2
  if output=$(GOALRAIL_BREW_SCENARIO="$scenario" PATH="$fake_bin:$PATH" "$subject"); then
    fail "$scenario unexpectedly succeeded"
  else
    status=$?
  fi
  [ "$status" -eq 4 ] || fail "$scenario returned exit $status instead of 4"
  printf '%s\n' "$output" | jq -e --arg code "$expected_code" '
    .schemaVersion == 1
    and .verdict == "BLOCKED"
    and .manager == "HOMEBREW"
    and .finding.code == $code
  ' >/dev/null || fail "$scenario omitted $expected_code"
}

no_change=$(run_success no-change NO_CHANGE)
printf '%s\n' "$no_change" | jq -e '
  .installedVersions == ["1.2.3"]
  and .availableVersion == "1.2.3"
  and .outdated == false
  and .outdatedExit == 0
' >/dev/null || fail "NO_CHANGE evidence is incomplete"

update=$(run_success update UPDATE_AVAILABLE)
printf '%s\n' "$update" | jq -e '
  .installedVersions == ["1.2.2"]
  and .availableVersion == "1.2.3"
  and .outdated == true
  and .outdatedExit == 1
' >/dev/null || fail "UPDATE_AVAILABLE did not normalize brew exit 1"

not_installed=$(run_success not-installed NOT_INSTALLED)
printf '%s\n' "$not_installed" | jq -e '
  .installedVersions == []
  and .availableVersion == "1.2.3"
  and .outdated == false
  and .outdatedExit == null
' >/dev/null || fail "NOT_INSTALLED evidence is incomplete"

run_blocked unsupported-status OUTDATED_FAILED
run_blocked info-noise INFO_INVALID
run_blocked info-missing-installed INFO_INVALID
run_blocked noise OUTDATED_INVALID
run_blocked exit-zero-with-entry STATE_CONTRADICTION
run_blocked exit-one-empty STATE_CONTRADICTION
run_blocked mismatched-current STATE_CONTRADICTION
run_blocked mismatched-installed STATE_CONTRADICTION

empty_bin="$fixture_root/empty-bin"
mkdir -p "$empty_bin"
if jq_missing=$(PATH="$empty_bin" /bin/sh "$subject"); then
  fail "missing jq unexpectedly succeeded"
else
  jq_missing_status=$?
fi
[ "$jq_missing_status" -eq 4 ] || fail "missing jq returned the wrong status"
printf '%s\n' "$jq_missing" | jq -e '
  .schemaVersion == 1
  and .verdict == "BLOCKED"
  and .manager == "HOMEBREW"
  and .finding.code == "JQ_MISSING"
' >/dev/null || fail "missing jq did not return structured evidence"

echo "HOMEBREW_UPDATE_STATE_TEST_OK scenarios=12 exit1=normalized contradictions=blocked"

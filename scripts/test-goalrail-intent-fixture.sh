#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
skill="$repo_root/fixtures/chat-native-development-intent/goalrail-intent/SKILL.md"

require_text() {
  expected=$1
  file=$2
  grep -F "$expected" "$file" >/dev/null || {
    echo "Goalrail intent fixture contract is missing from $file: $expected" >&2
    return 1
  }
}

reject_representative_fixed_models() {
  file=$1
  pattern='gpt-[[:alnum:]._-]+|claude-[[:alnum:]._-]+|gemini-[[:alnum:]._-]+|(^|[^[:alnum:]_])(Luna|Terra|Sol|Opus|Sonnet|Haiku)([^[:alnum:]_]|$)'

  set +e
  grep -E "$pattern" "$file" >/dev/null
  status=$?
  set -e

  case $status in
    0)
      echo "Goalrail intent fixture hard-codes a representative model identifier: $file" >&2
      return 1
      ;;
    1)
      return 0
      ;;
    *)
      echo "Goalrail intent fixture model scan failed for $file" >&2
      return 1
      ;;
  esac
}

validate_contract() {
  file=$1

  test -s "$file" || {
    echo "Goalrail intent fixture is missing or empty: $file" >&2
    return 1
  }

  sed -n '1,8p' "$file" | grep -F 'name: goalrail-intent' >/dev/null || {
    echo "Goalrail intent fixture has the wrong identity: $file" >&2
    return 1
  }

  require_text 'Use ordinary Codex chat as the runtime.' "$file" || return 1
  require_text 'For a decision-bearing change, compare two or three current viable approaches' "$file" || return 1
  require_text 'Treat the native or no-build path as a real' "$file" || return 1
  require_text 'Do not prescribe a fixed model, reasoning effort, role chain, or agent count.' "$file" || return 1
  require_text "Persist only owner-confirmed durable decisions in the project's existing" "$file" || return 1
  require_text 'otherwise mark the pair `INVALID` and do not count it' "$file" || return 1
  reject_representative_fixed_models "$file" || return 1
}

validate_contract "$skill"

trial_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-intent-fixture.XXXXXX")
trap 'rm -rf -- "$trial_dir"' EXIT HUP INT TERM

missing_native="$trial_dir/missing-native.md"
sed '/Treat the native or no-build path as a real/d' "$skill" >"$missing_native"
if validate_contract "$missing_native" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted a missing native/no-build candidate" >&2
  exit 1
fi

for fixed_identifier in \
  'Always use gpt-5.6-sol for architecture.' \
  'Always use claude-opus-5 for architecture.' \
  'Always use gemini-3-pro for architecture.'; do
  fixed_model="$trial_dir/fixed-model.md"
  {
    printf '%s\n' "$fixed_identifier"
    sed -n '1,$p' "$skill"
  } >"$fixed_model"
  if validate_contract "$fixed_model" >/dev/null 2>&1; then
    echo "Goalrail intent fixture accepted a representative hard-coded model: $fixed_identifier" >&2
    exit 1
  fi
done

scan_error="$trial_dir/model-scan-error.txt"
if reject_representative_fixed_models "$trial_dir/missing.md" 2>"$scan_error"; then
  echo "Goalrail intent fixture accepted a model-scan error" >&2
  exit 1
fi
grep -F 'model scan failed' "$scan_error" >/dev/null || {
  echo "Goalrail intent fixture did not distinguish a model-scan error" >&2
  exit 1
}

echo "GOALRAIL_INTENT_FIXTURE_CONTRACT_OK scope=structure-declared-boundary-representative-identifiers"

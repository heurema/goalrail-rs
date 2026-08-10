#!/bin/sh

set -eu

fail() {
  echo "skills boundary test failed: $*" >&2
  exit 1
}

source_root=$(git rev-parse --show-toplevel)
subject="$source_root/scripts/check-skills-boundary.rb"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-skills-boundary-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

make_pending_fixture() {
  fixture_root=$1
  mkdir -p "$fixture_root/crates/gr-inspect-codex/src"
  printf '%s\n' '// Boundary extraction is pending.' \
    >"$fixture_root/crates/gr-inspect-codex/src/skills.rs"
}

make_complete_fixture() {
  fixture_root=$1
  source_dir="$fixture_root/crates/gr-inspect-codex/src/skills"
  mkdir -p "$source_dir"
  cat >"$fixture_root/crates/gr-inspect-codex/src/skills.rs" <<'RUST'
mod model;
mod catalog;
mod history;
mod assessment;
mod presentation;
RUST
  cat >"$source_dir/model.rs" <<'RUST'
pub(super) struct Evidence;
RUST
  cat >"$source_dir/catalog.rs" <<'RUST'
use super::model::Evidence;

pub(super) fn acquire() -> Evidence {
    Evidence
}
RUST
  cat >"$source_dir/history.rs" <<'RUST'
use super::model::Evidence;

pub(super) fn acquire() -> Evidence {
    Evidence
}
RUST
  cat >"$source_dir/assessment.rs" <<'RUST'
use super::model::Evidence;

// Forbidden APIs in comments and strings are not dependencies or effects:
// std::fs::read_to_string and Command::new.
const NOTE: &str = "SystemTime and println! are inert here";

pub(super) struct Assessment(Evidence);
RUST
  cat >"$source_dir/presentation.rs" <<'RUST'
use super::assessment::Assessment;

pub(super) struct View(Assessment);
RUST
}

run_check() {
  fixture_root=$1
  output=$2
  GOALRAIL_SKILLS_BOUNDARY_ROOT="$fixture_root" \
    ruby "$subject" >"$output" 2>&1
}

pending="$test_root/pending"
make_pending_fixture "$pending"
pending_output="$test_root/pending.out"
run_check "$pending" "$pending_output" || {
  sed -n '1,100p' "$pending_output" >&2
  fail "the pre-extraction state should remain reviewable"
}
grep -Fq 'AD-6: REVIEW - accepted boundary is not implemented' "$pending_output" ||
  fail "the pending fixture omitted the explicit AD-6 review"

positive="$test_root/positive"
make_pending_fixture "$positive"
make_complete_fixture "$positive"
positive_output="$test_root/positive.out"
run_check "$positive" "$positive_output" || {
  sed -n '1,100p' "$positive_output" >&2
  fail "the allowed source topology was rejected"
}
grep -Fq 'AD-6: REVIEW - source-level contract conforms; semantic module graph pending' \
  "$positive_output" ||
  fail "the positive fixture overclaimed or omitted its provisional result"
if grep -Fq 'AD-6: PASS' "$positive_output"; then
  fail "the source-only positive fixture claimed semantic proof"
fi

partial="$test_root/partial"
make_pending_fixture "$partial"
mkdir -p "$partial/crates/gr-inspect-codex/src/skills"
printf '%s\n' 'pub(super) struct Evidence;' \
  >"$partial/crates/gr-inspect-codex/src/skills/model.rs"
printf '%s\n' 'mod model;' \
  >>"$partial/crates/gr-inspect-codex/src/skills.rs"
partial_output="$test_root/partial.out"
if run_check "$partial" "$partial_output"; then
  fail "a partial module topology passed"
fi
grep -Fq 'AD-6: FAILED - accepted boundary has a partial or unclassified module topology' \
  "$partial_output" ||
  fail "the partial topology failure was unclear"

spoofed="$test_root/spoofed-declarations"
make_pending_fixture "$spoofed"
make_complete_fixture "$spoofed"
cat >"$spoofed/crates/gr-inspect-codex/src/skills.rs" <<'RUST'
/*
mod model;
mod catalog;
mod history;
mod assessment;
mod presentation;
*/
mod hidden {
    const CLOSE: char = '}';
    const BYTE_CLOSE: u8 = b'}';
    mod model;
    mod catalog;
    mod history;
    mod assessment;
    mod presentation;
}
RUST
spoofed_output="$test_root/spoofed.out"
if run_check "$spoofed" "$spoofed_output"; then
  fail "comments or nested modules spoofed the required top-level declarations"
fi
grep -Fq 'missing module declarations: model, catalog, history, assessment, presentation' \
  "$spoofed_output" ||
  fail "the declaration-spoof fixture did not fail closed"

conditional="$test_root/conditional-declarations"
make_pending_fixture "$conditional"
make_complete_fixture "$conditional"
cat >"$conditional/crates/gr-inspect-codex/src/skills.rs" <<'RUST'
#[cfg(test)]
mod model;
#[cfg(any())]
mod catalog;
#[cfg(test)]
mod history;
#[cfg(any())]
mod assessment;
#[cfg(test)]
mod presentation;
RUST
conditional_output="$test_root/conditional.out"
if run_check "$conditional" "$conditional_output"; then
  fail "conditional modules spoofed the required production topology"
fi
grep -Fq 'conditional or attributed module declarations are not accepted' \
  "$conditional_output" ||
  fail "the conditional declaration fixture did not fail closed"

forbidden="$test_root/forbidden"
make_pending_fixture "$forbidden"
make_complete_fixture "$forbidden"
cat >>"$forbidden/crates/gr-inspect-codex/src/skills/history.rs" <<'RUST'

use super::presentation::View;

pub(super) fn forbidden_view() -> Option<View> {
    None
}
RUST
forbidden_output="$test_root/forbidden.out"
if run_check "$forbidden" "$forbidden_output"; then
  fail "history was allowed to depend on presentation"
fi
grep -Fq 'AD-6 forbidden edge: history -> presentation' "$forbidden_output" ||
  fail "the forbidden-edge fixture did not name the rejected edge"

for category in process environment filesystem clock rendering; do
  impure="$test_root/impure-$category"
  make_pending_fixture "$impure"
  make_complete_fixture "$impure"
  case "$category" in
    process)
      violation='fn forbidden() { let _ = std::process::Command::new("codex"); }'
      expected='process access'
      ;;
    environment)
      violation='fn forbidden() { let _ = std::env::var("GOALRAIL_TEST"); }'
      expected='environment access'
      ;;
    filesystem)
      violation='fn forbidden() { let _ = std::fs::read_to_string("evidence.json"); }'
      expected='filesystem access'
      ;;
    clock)
      violation='fn forbidden() { let _ = std::time::SystemTime::now(); }'
      expected='clock access'
      ;;
    rendering)
      violation='fn forbidden() { println!("assessment"); }'
      expected='rendering'
      ;;
  esac
  printf '\n%s\n' "$violation" \
    >>"$impure/crates/gr-inspect-codex/src/skills/assessment.rs"
  impure_output="$test_root/impure-$category.out"
  if run_check "$impure" "$impure_output"; then
    fail "assessment was allowed to perform $category work"
  fi
  grep -Fq "AD-6 assessment purity violation: $expected" "$impure_output" ||
    fail "the purity fixture did not identify $expected"
done

echo "Skills boundary tests passed"

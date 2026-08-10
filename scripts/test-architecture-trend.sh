#!/bin/sh

set -eu

fail() {
  echo "architecture trend test failed: $*" >&2
  exit 1
}

source_root=$(git rev-parse --show-toplevel)
subject="$source_root/scripts/check-architecture-trend.rb"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-architecture-trend-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

write_fixture() {
  fixture_root=$1
  hotspot_lines=$2
  hotspot_items=$3
  ruby -e '
    root = ARGV.fetch(0)
    hotspot_lines = Integer(ARGV.fetch(1))
    hotspot_items = Integer(ARGV.fetch(2))
    source = File.join(root, "crates", "example", "src")
    Dir.mkdir(File.join(root, "crates")) unless Dir.exist?(File.join(root, "crates"))
    Dir.mkdir(File.join(root, "crates", "example")) unless Dir.exist?(File.join(root, "crates", "example"))
    Dir.mkdir(source) unless Dir.exist?(source)
    (10..16).each_with_index do |lines, index|
      File.write(
        File.join(source, "peer_#{index}.rs"),
        Array.new(lines, "// peer\n").join,
      )
    end
    function_forms = [
      "fn operation_0() {}\n",
      "unsafe fn operation_1() {}\n",
      "extern \"C\" fn operation_2() {}\n",
      "pub unsafe extern \"C\" fn operation_3() {}\n",
    ]
    hotspot = Array.new(hotspot_items) do |index|
      function_forms.fetch(index, "fn operation_#{index}() {}\n")
    end
    hotspot.concat(Array.new(hotspot_lines - hotspot.length, "// hotspot\n"))
    File.write(File.join(source, "aggregate.rs"), hotspot.join)
  ' "$fixture_root" "$hotspot_lines" "$hotspot_items"
}

commit_fixture() {
  fixture_root=$1
  message=$2
  git -C "$fixture_root" add .
  git -C "$fixture_root" commit -q -m "$message"
}

growing="$test_root/growing"
mkdir -p "$growing"
git -C "$growing" init -q
git -C "$growing" config user.email fixture@example.invalid
git -C "$growing" config user.name "Architecture Fixture"
write_fixture "$growing" 40 2
commit_fixture "$growing" "first revision"
write_fixture "$growing" 60 3
commit_fixture "$growing" "second revision"
write_fixture "$growing" 80 4
commit_fixture "$growing" "third revision"

growing_output="$test_root/growing.out"
GOALRAIL_ARCHITECTURE_TREND_ROOT="$growing" ruby "$subject" >"$growing_output" 2>&1 ||
  fail "the advisory review signal returned a failure exit"
for marker in \
  '- REVIEW crates/example/src/aggregate.rs' \
  'source_lines: 40 -> 60 -> 80' \
  'top_level_items: 2 -> 3 -> 4' \
  'crate_tukey_upper_fence:' \
  'Architecture trend v0: REVIEW'; do
  grep -Fq -- "$marker" "$growing_output" || {
    sed -n '1,100p' "$growing_output" >&2
    fail "the growing hotspot receipt omitted $marker"
  }
done

single_revision="$test_root/single-revision"
mkdir -p "$single_revision"
git -C "$single_revision" init -q
git -C "$single_revision" config user.email fixture@example.invalid
git -C "$single_revision" config user.name "Architecture Fixture"
write_fixture "$single_revision" 80 4
commit_fixture "$single_revision" "single revision"

single_output="$test_root/single.out"
GOALRAIL_ARCHITECTURE_TREND_ROOT="$single_revision" ruby "$subject" >"$single_output" 2>&1 ||
  fail "the observation-only signal returned a failure exit"
grep -Fq -- '- OBSERVE crates/example/src/aggregate.rs' "$single_output" ||
  fail "a structural outlier without repeated growth was not observed"
grep -Fq 'Architecture trend v0: OBSERVE' "$single_output" ||
  fail "an observation-only fixture produced the wrong aggregate"
if grep -Fq -- '- REVIEW ' "$single_output"; then
  fail "a single revision was promoted to REVIEW"
fi

size_only="$test_root/size-only"
mkdir -p "$size_only"
git -C "$size_only" init -q
git -C "$size_only" config user.email fixture@example.invalid
git -C "$size_only" config user.name "Architecture Fixture"
write_fixture "$size_only" 40 2
commit_fixture "$size_only" "first size revision"
write_fixture "$size_only" 60 2
commit_fixture "$size_only" "second size revision"
write_fixture "$size_only" 80 2
commit_fixture "$size_only" "third size revision"

size_only_output="$test_root/size-only.out"
GOALRAIL_ARCHITECTURE_TREND_ROOT="$size_only" ruby "$subject" >"$size_only_output" 2>&1 ||
  fail "the size-only observation returned a failure exit"
grep -Fq -- '- OBSERVE crates/example/src/aggregate.rs' "$size_only_output" ||
  fail "size growth without item growth was not kept as an observation"
if grep -Fq -- '- REVIEW ' "$size_only_output"; then
  fail "size growth alone was promoted to REVIEW"
fi

worktree_growth="$test_root/worktree-growth"
mkdir -p "$worktree_growth"
git -C "$worktree_growth" init -q
git -C "$worktree_growth" config user.email fixture@example.invalid
git -C "$worktree_growth" config user.name "Architecture Fixture"
write_fixture "$worktree_growth" 40 2
commit_fixture "$worktree_growth" "first committed revision"
write_fixture "$worktree_growth" 60 3
commit_fixture "$worktree_growth" "second committed revision"
write_fixture "$worktree_growth" 80 4

worktree_output="$test_root/worktree.out"
GOALRAIL_ARCHITECTURE_TREND_ROOT="$worktree_growth" ruby "$subject" >"$worktree_output" 2>&1 ||
  fail "the worktree review signal returned a failure exit"
grep -Fq -- '- REVIEW crates/example/src/aggregate.rs' "$worktree_output" ||
  fail "uncommitted cumulative growth was not reviewed"
grep -Fq 'WORKTREE' "$worktree_output" ||
  fail "the receipt did not identify its uncommitted observation"

echo "Architecture trend tests passed"

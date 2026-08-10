#!/bin/sh

set -eu

fail() {
  echo "verification receipt test failed: $*" >&2
  exit 1
}

source_root=$(git rev-parse --show-toplevel)
subject="$source_root/scripts/rust-verification-receipts.sh"
installer="$source_root/scripts/install-git-hooks.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-receipt-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

repo="$test_root/repo"
mkdir -p "$repo/src" "$repo/docs"
cd "$repo"
git init -q
git config user.name "Goalrail Receipt Test"
git config user.email "receipt-test@example.invalid"

printf '%s\n' '[package]' 'name = "fixture"' 'version = "0.1.0"' >Cargo.toml
printf '%s\n' 'fn value() -> u8 { 1 }' >src/lib.rs
printf '%s\n' 'min_version = "2025.8.11"' >mise.toml
printf '%s\n' '# Fixture' >docs/readme.md
git add .
git commit -qm "initial fixture"
initial=$(git rev-parse HEAD)

fake_bin="$test_root/bin"
mkdir -p "$fake_bin"
fake_mise_log="$test_root/mise.log"
# The generated fixture must expand these variables when it runs, not here.
# shellcheck disable=SC2016
{
  echo '#!/bin/sh'
  echo 'echo "$*" >>"$GOALRAIL_TEST_MISE_LOG"'
  echo 'if test "${GOALRAIL_TEST_FAIL_COMMAND:-}" = "$*"; then exit 9; fi'
  echo 'if test "${GOALRAIL_TEST_MUTATE_FILE:-}" != "" && test "$*" = "run ci"; then echo changed >>"$GOALRAIL_TEST_MUTATE_FILE"; fi'
} >"$fake_bin/mise"
chmod +x "$fake_bin/mise"

printf '%s\n' 'Documentation only.' >>docs/readme.md
git add docs/readme.md
git commit -qm "document fixture"
docs_head=$(git rev-parse HEAD)
if "$subject" check "$initial" "$docs_head" >"$test_root/docs.out" 2>&1; then
  fail "documentation change passed without exact-tree CI evidence"
fi
grep -q "mise run verify:ci-state -- $initial $docs_head" \
  "$test_root/docs.out" || {
  sed -n '1,80p' "$test_root/docs.out" >&2
  fail "CI-only mismatch did not print its exact recovery command"
}
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-ci "$initial" "$docs_head" >/dev/null
"$subject" check "$initial" "$docs_head" >/dev/null ||
  fail "CI receipt did not cover a documentation-only change"

printf '%s\n' 'fn value() -> u8 { 2 }' >src/lib.rs
first_diff="$test_root/first.diff"
git diff --binary >"$first_diff"
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-milestone "$first_diff" >/dev/null
git add src/lib.rs
git commit -qm "change fixture value"
first_rust_head=$(git rev-parse HEAD)
"$subject" check "$initial" "$first_rust_head" >/dev/null ||
  fail "one valid receipt did not cover its Rust change"

printf '%s\n' 'More documentation.' >>docs/readme.md
git add docs/readme.md
git commit -qm "extend fixture docs"
docs_after_receipt=$(git rev-parse HEAD)
if "$subject" check "$initial" "$docs_after_receipt" \
  >"$test_root/docs-after.out" 2>&1; then
  fail "changed CI tree reused stale CI evidence"
fi
grep -q "mise run verify:ci-state -- $first_rust_head $docs_after_receipt" \
  "$test_root/docs-after.out" ||
  fail "stale CI tree unnecessarily requested mutation testing"
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-ci "$first_rust_head" "$docs_after_receipt" >/dev/null
"$subject" check "$initial" "$docs_after_receipt" >/dev/null ||
  fail "fresh CI receipt did not preserve prior mutation evidence"

printf '%s\n' 'fn value() -> u8 { 3 }' >src/lib.rs
second_diff="$test_root/second.diff"
git diff --binary >"$second_diff"
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-milestone "$second_diff" >/dev/null
git add src/lib.rs
git commit -qm "change fixture value again"
second_rust_head=$(git rev-parse HEAD)
"$subject" check "$initial" "$second_rust_head" >/dev/null ||
  fail "two valid receipts did not form a mutation-state chain"

printf '%s\n' 'fn value() -> u8 { 4 }' >src/lib.rs
stale_diff="$test_root/stale.diff"
git diff --binary >"$stale_diff"
printf '%s\n' 'fn value() -> u8 { 5 }' >src/lib.rs
if GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-milestone "$stale_diff" \
  >"$test_root/stale.out" 2>&1; then
  fail "stale goal diff produced a receipt"
fi
grep -q "does not match" "$test_root/stale.out" ||
  fail "stale goal diff did not explain the mismatch"

git add src/lib.rs
git commit -qm "add unverified fixture change"
unverified_head=$(git rev-parse HEAD)
if "$subject" check "$initial" "$unverified_head" \
  >"$test_root/unverified.out" 2>&1; then
  fail "unverified Rust change passed the receipt check"
fi
grep -q "mise run verify:outgoing-rust -- $initial $unverified_head" \
  "$test_root/unverified.out" ||
  fail "missing mutation receipt did not print the exact recovery command"

GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-outgoing "$initial" "$unverified_head" >/dev/null
grep -q '^run ci$' "$fake_mise_log" ||
  fail "outgoing recovery did not run CI"
grep -q '^run mutants:diff$' "$fake_mise_log" ||
  fail "outgoing recovery did not run diff-scoped mutations"
"$subject" check "$initial" "$unverified_head" >/dev/null ||
  fail "outgoing recovery did not record usable evidence"

mkdir -p .githooks scripts
cp "$source_root/.githooks/pre-push" .githooks/pre-push
cp "$subject" scripts/rust-verification-receipts.sh
cp "$installer" scripts/install-git-hooks.sh
git add .githooks scripts
git commit -qm "add receipt hook fixture"
local_head=$(git rev-parse HEAD)
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-ci "$unverified_head" "$local_head" >/dev/null

bare_remote="$test_root/remote.git"
git init -q --bare "$bare_remote"
git remote add origin "$bare_remote"
git push -q origin "$initial:refs/heads/main"
legacy_pre_commit=$(git rev-parse --git-path hooks/pre-commit)
{
  echo '#!/bin/sh'
  echo 'exit 0'
} >"$legacy_pre_commit"
chmod +x "$legacy_pre_commit"
if "$installer" >"$test_root/installer-block.out" 2>&1; then
  fail "hook installer disabled an unrelated executable hook"
fi
grep -q "would be disabled" "$test_root/installer-block.out" ||
  fail "hook installer did not identify the conflicting hook"
unlink "$legacy_pre_commit"
"$installer" >/dev/null
expected_hooks=$(git rev-parse --show-toplevel)/.githooks
test "$(git config --local --get core.hooksPath)" = "$expected_hooks" ||
  fail "hook installer did not select the tracked hook path"
git push -q origin "HEAD:refs/heads/main" ||
  fail "native pre-push hook did not accept valid receipts"
remote_head=$(git --git-dir="$bare_remote" rev-parse refs/heads/main)
test "$remote_head" = "$local_head" ||
  fail "local hook smoke did not update the temporary remote"

empty_remote="$test_root/empty.git"
git init -q --bare "$empty_remote"
git remote add empty-origin "$empty_remote"
empty_tree=$(git mktree </dev/null)
if git push -q empty-origin "HEAD:refs/heads/main" \
  >"$test_root/empty-push.out" 2>&1; then
  fail "first push to an empty remote passed without root evidence"
fi
grep -q "mise run verify:outgoing-rust -- $empty_tree $local_head" \
  "$test_root/empty-push.out" ||
  fail "empty-remote push did not print a usable verification command"
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-outgoing "$empty_tree" "$local_head" >/dev/null
git push -q empty-origin "HEAD:refs/heads/main" ||
  fail "verified first push to an empty remote remained blocked"

git switch -qc docs-branch
printf '%s\n' 'Branch documentation.' >>docs/readme.md
git add docs/readme.md
git commit -qm "add branch docs"
docs_branch_head=$(git rev-parse HEAD)
GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
  "$subject" verify-ci "$local_head" "$docs_branch_head" >/dev/null

git switch -qc unverified-branch "$local_head"
printf '%s\n' 'fn value() -> u8 { 6 }' >src/lib.rs
git add src/lib.rs
git commit -qm "add unverified branch change"
unverified_branch_head=$(git rev-parse HEAD)

if git push -q origin \
  "$docs_branch_head:refs/heads/docs-branch" \
  "$unverified_branch_head:refs/heads/unverified-branch"; then
  fail "multi-ref push skipped an unverified Rust ref"
fi
if git --git-dir="$bare_remote" show-ref --verify --quiet \
  refs/heads/docs-branch; then
  fail "failed multi-ref pre-push partially updated the remote"
fi

git push -q origin "$docs_branch_head:refs/heads/docs-branch"
git push -q origin ":refs/heads/docs-branch" ||
  fail "branch deletion was not treated as a no-content update"

git switch -qc remote-doc-base "$initial"
printf '%s\n' 'Remote documentation.' >>docs/readme.md
git add docs/readme.md
git commit -qm "add remote-side docs"
remote_doc_head=$(git rev-parse HEAD)
git switch -qc rebased-rust "$remote_doc_head"
printf '%s\n' 'fn value() -> u8 { 5 }' >src/lib.rs
git add src/lib.rs
git commit -qm "replay verified Rust state"
rebased_rust_head=$(git rev-parse HEAD)
if "$subject" check "$remote_doc_head" "$rebased_rust_head" \
  >"$test_root/rebased.out" 2>&1; then
  fail "rebased Rust state reused mutation evidence from a different full tree"
fi
grep -q "mise run verify:outgoing-rust -- $remote_doc_head $rebased_rust_head" \
  "$test_root/rebased.out" ||
  fail "rebased Rust state did not print fail-closed recovery"

linked_worktree="$test_root/linked-worktree"
git worktree add -qb linked-receipt "$linked_worktree" "$local_head"
printf '%s\n' 'Linked worktree documentation.' >>"$linked_worktree/docs/readme.md"
git -C "$linked_worktree" add docs/readme.md
git -C "$linked_worktree" commit -qm "add linked worktree docs"
linked_head=$(git -C "$linked_worktree" rev-parse HEAD)
(
  cd "$linked_worktree"
  GOALRAIL_TEST_MISE_LOG="$fake_mise_log" PATH="$fake_bin:$PATH" \
    "$subject" verify-ci "$local_head" "$linked_head" >/dev/null
)
"$subject" check "$local_head" "$linked_head" >/dev/null ||
  fail "receipt recorded in a linked worktree was not shared"

git switch -qc verification-failure "$local_head"
printf '%s\n' 'fn value() -> u8 { 7 }' >src/lib.rs
failure_diff="$test_root/failure.diff"
git diff --binary >"$failure_diff"
receipt_dir=$(git rev-parse --git-common-dir)/goalrail/rust-verification-receipts
before_count=$(find "$receipt_dir" -type f -name '*.receipt' | wc -l | tr -d ' ')
if GOALRAIL_TEST_MISE_LOG="$fake_mise_log" \
  GOALRAIL_TEST_FAIL_COMMAND='run ci' PATH="$fake_bin:$PATH" \
  "$subject" verify-milestone "$failure_diff" \
  >"$test_root/failed-ci.out" 2>&1; then
  fail "failed CI produced a milestone receipt"
fi
after_count=$(find "$receipt_dir" -type f -name '*.receipt' | wc -l | tr -d ' ')
test "$before_count" = "$after_count" ||
  fail "failed CI changed the receipt set"

if GOALRAIL_TEST_MISE_LOG="$fake_mise_log" \
  GOALRAIL_TEST_MUTATE_FILE="$repo/docs/readme.md" PATH="$fake_bin:$PATH" \
  "$subject" verify-milestone "$failure_diff" \
  >"$test_root/raced-ci.out" 2>&1; then
  fail "checkout mutation during verification produced a receipt"
fi
grep -q "changed during milestone verification" "$test_root/raced-ci.out" ||
  fail "checkout mutation did not report a stale verification state"
final_count=$(find "$receipt_dir" -type f -name '*.receipt' | wc -l | tr -d ' ')
test "$before_count" = "$final_count" ||
  fail "checkout mutation during verification changed the receipt set"

echo "Rust verification receipt tests passed"

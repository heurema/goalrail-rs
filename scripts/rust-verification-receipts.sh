#!/bin/sh

set -eu

MAX_RECEIPT_STATES=512
MAX_RECOVERY_COMMITS=256

usage() {
  cat >&2 <<'EOF'
Usage:
  rust-verification-receipts.sh verify-milestone <goal-diff>
  rust-verification-receipts.sh verify-ci <base> [<head>]
  rust-verification-receipts.sh check <remote-base> <local-head> [<local-ref>]
  rust-verification-receipts.sh verify-outgoing <remote-base> [<local-head>]
EOF
  exit 2
}

die() {
  echo "Goalrail verification receipt: $*" >&2
  exit 1
}

repo_root=$(git rev-parse --show-toplevel 2>/dev/null) ||
  die "not inside a Git worktree"
cd "$repo_root"

git_dir=$(git rev-parse --git-common-dir)
case "$git_dir" in
  /*) ;;
  *) git_dir="$repo_root/$git_dir" ;;
esac

receipt_root="$git_dir/goalrail/rust-verification-receipts"
temp_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-receipts.XXXXXX")
trap 'rm -rf "$temp_root"' EXIT HUP INT TERM

new_index_path() {
  index_path=$(mktemp "$temp_root/index.XXXXXX")
  rm -f "$index_path"
  printf '%s\n' "$index_path"
}

rust_hash_from_index() {
  GIT_INDEX_FILE="$1" git ls-files -s -z -- '*.rs' |
    git hash-object --stdin
}

rust_hash_for_worktree() {
  index_path=$(new_index_path)
  GIT_INDEX_FILE="$index_path" git read-tree HEAD
  GIT_INDEX_FILE="$index_path" git add -A -- .
  rust_hash_from_index "$index_path"
}

rust_hash_after_diff() {
  base=$1
  diff_path=$2
  index_path=$(new_index_path)
  GIT_INDEX_FILE="$index_path" git read-tree "$base"
  GIT_INDEX_FILE="$index_path" git apply --cached --check \
    --whitespace=nowarn "$diff_path"
  GIT_INDEX_FILE="$index_path" git apply --cached \
    --whitespace=nowarn "$diff_path"
  rust_hash_from_index "$index_path"
}

tree_hash_for_worktree() {
  index_path=$(new_index_path)
  GIT_INDEX_FILE="$index_path" git read-tree HEAD
  GIT_INDEX_FILE="$index_path" git add -A -- .
  GIT_INDEX_FILE="$index_path" git write-tree
}

resolve_commit() {
  git rev-parse --verify "$1^{commit}" 2>/dev/null
}

resolve_tree() {
  git rev-parse --verify "$1^{tree}" 2>/dev/null
}

empty_tree() {
  git mktree </dev/null
}

is_zero_oid() {
  case "$1" in
    ''|*[!0]*) return 1 ;;
    *) return 0 ;;
  esac
}

is_object_hash() {
  case "$1" in
    ''|*[!0-9a-f]*) return 1 ;;
    *) return 0 ;;
  esac
}

receipt_field() {
  field=$1
  receipt=$2
  sed -n "s/^${field}=//p" "$receipt"
}

write_receipt() {
  evidence_kind=$1
  base_object=$2
  evidence_hash=$3
  base_state=$4
  state=$5
  ci_tree=$6

  verified_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  mkdir -p "$receipt_root"
  receipt="$receipt_root/${base_object}-${evidence_hash}-${ci_tree}.receipt"
  temporary_receipt=$(mktemp "$receipt_root/.receipt.XXXXXX")
  {
    echo "schemaVersion=3"
    echo "evidenceKind=$evidence_kind"
    echo "baseObject=$base_object"
    echo "evidenceHash=$evidence_hash"
    echo "baseState=$base_state"
    echo "state=$state"
    echo "ciTree=$ci_tree"
    echo "verifiedAt=$verified_at"
  } >"$temporary_receipt"
  mv "$temporary_receipt" "$receipt"

  echo "Recorded Rust verification receipt: $receipt"
  echo "Evidence kind: $evidence_kind"
  echo "Verified tree: $state"
}

record_milestone_receipt() {
  base=$1
  diff_path=$2

  test -s "$diff_path" ||
    die "goal-scoped diff is missing or empty: $diff_path"
  base_tree=$(resolve_tree "$base") || die "invalid receipt base: $base"
  expected_rust=$(rust_hash_after_diff "$base" "$diff_path") ||
    die "goal-scoped diff does not apply cleanly to $base"
  worktree_rust=$(rust_hash_for_worktree)
  test "$expected_rust" = "$worktree_rust" ||
    die "goal-scoped diff does not match current Rust files; include new files with git add -N"

  diff_hash=$(git hash-object "$diff_path")
  verified_tree=$(tree_hash_for_worktree)
  write_receipt milestone "$base" "$diff_hash" "$base_tree" \
    "$verified_tree" "$verified_tree"

  diff_tree_index=$(new_index_path)
  GIT_INDEX_FILE="$diff_tree_index" git read-tree "$base"
  GIT_INDEX_FILE="$diff_tree_index" git apply --cached \
    --whitespace=nowarn "$diff_path"
  diff_tree=$(GIT_INDEX_FILE="$diff_tree_index" git write-tree)
  if test "$diff_tree" != "$verified_tree"; then
    echo "Receipt note: commit every current unignored worktree change to preserve exact-tree CI evidence." >&2
  fi
}

record_ci_receipt() {
  base=$1
  head=$2
  base_tree=$(resolve_tree "$base") || die "invalid CI base: $base"
  head_tree=$(resolve_tree "$head") || die "invalid CI head: $head"
  evidence_hash=$(git diff --binary "$base" "$head" | git hash-object --stdin)
  write_receipt ci "$base" "$evidence_hash" "$base_tree" "$head_tree" \
    "$head_tree"
}

verify_current_milestone() {
  test "$#" -eq 1 || usage
  diff_path=$1
  test -s "$diff_path" ||
    die "goal-scoped diff is missing or empty: $diff_path"
  base=$(resolve_commit HEAD) || die "cannot resolve HEAD"
  expected_rust=$(rust_hash_after_diff "$base" "$diff_path") ||
    die "goal-scoped diff does not apply cleanly to $base"
  before_rust=$(rust_hash_for_worktree)
  test "$expected_rust" = "$before_rust" ||
    die "goal-scoped diff does not match current Rust files; include new files with git add -N"
  before_tree=$(tree_hash_for_worktree)

  mise run ci
  GOALRAIL_MUTATION_DIFF="$diff_path" mise run mutants:diff

  after_tree=$(tree_hash_for_worktree)
  test "$before_tree" = "$after_tree" ||
    die "working tree changed during milestone verification; no receipt recorded"
  record_milestone_receipt "$base" "$diff_path"
}

verify_ci_state() {
  test "$#" -ge 1 && test "$#" -le 2 || usage
  base=$(git rev-parse --verify "$1" 2>/dev/null) ||
    die "invalid CI base: $1"
  head_ref=${2:-HEAD}
  head=$(resolve_commit "$head_ref") || die "invalid CI head: $head_ref"
  current_head=$(resolve_commit HEAD) || die "cannot resolve HEAD"
  test "$head" = "$current_head" ||
    die "CI verification must target the checked-out HEAD: $current_head"
  git diff --quiet "$base" "$head" -- '*.rs' ||
    die "CI-only verification cannot cover Rust source changes; use verify:outgoing-rust"

  before_tree=$(tree_hash_for_worktree)
  committed_tree=$(resolve_tree "$head")
  test "$before_tree" = "$committed_tree" ||
    die "working tree changes affect the CI tree; commit or isolate them first"

  mise run ci

  after_tree=$(tree_hash_for_worktree)
  test "$before_tree" = "$after_tree" ||
    die "working tree changed during CI verification; no receipt recorded"
  record_ci_receipt "$base" "$head"
}

find_new_branch_base() {
  remote_name=$1
  head=$2
  best_base=
  best_distance=

  for remote_commit in $(
    git for-each-ref --format='%(objectname)' "refs/remotes/$remote_name"
  ); do
    git cat-file -e "$remote_commit^{commit}" 2>/dev/null || continue
    git merge-base --is-ancestor "$remote_commit" "$head" || continue
    distance=$(git rev-list --count "$remote_commit..$head")
    if test -z "$best_distance" || test "$distance" -lt "$best_distance"; then
      best_base=$remote_commit
      best_distance=$distance
    fi
  done

  if test -n "$best_base"; then
    printf '%s\n' "$best_base"
  else
    empty_tree
  fi
}

receipt_is_valid() {
  receipt=$1
  test "$(receipt_field schemaVersion "$receipt")" = "3" || return 1
  case "$(receipt_field evidenceKind "$receipt")" in
    milestone|ci) ;;
    *) return 1 ;;
  esac
  for field in baseObject evidenceHash baseState state ciTree; do
    value=$(receipt_field "$field" "$receipt")
    is_object_hash "$value" || return 1
  done
}

has_ci_receipt() {
  expected_tree=$1
  for receipt in "$receipt_root"/*.receipt; do
    test -f "$receipt" || continue
    receipt_is_valid "$receipt" || continue
    test "$(receipt_field ciTree "$receipt")" = "$expected_tree" && return 0
  done
  return 1
}

state_path_exists() {
  from_state=$1
  to_state=$2
  test "$from_state" != "$to_state" || return 0

  graph_dir=$(mktemp -d "$temp_root/graph.XXXXXX")
  queue="$graph_dir/queue"
  visited="$graph_dir/visited"
  mkdir -p "$visited"
  printf '%s\n' "$to_state" >"$queue"
  queue_line=1
  visited_count=0

  while current_state=$(sed -n "${queue_line}p" "$queue"); do
    test -n "$current_state" || break
    queue_line=$((queue_line + 1))
    test "$current_state" = "$from_state" && return 0
    test ! -f "$visited/$current_state" || continue

    visited_count=$((visited_count + 1))
    if test "$visited_count" -gt "$MAX_RECEIPT_STATES"; then
      die "receipt graph exceeds $MAX_RECEIPT_STATES states; archive $receipt_root and reverify the outgoing range"
    fi
    : >"$visited/$current_state"

    for receipt in "$receipt_root"/*.receipt; do
      test -f "$receipt" || continue
      receipt_is_valid "$receipt" || continue
      test "$(receipt_field state "$receipt")" = "$current_state" ||
        continue
      base_state=$(receipt_field baseState "$receipt")
      test -f "$visited/$base_state" || echo "$base_state" >>"$queue"
    done
  done

  return 1
}

find_ci_recovery_base() {
  from_state=$1
  head=$2
  for candidate in $(
    git rev-list --first-parent --max-count="$MAX_RECOVERY_COMMITS" "$head"
  ); do
    test "$candidate" != "$head" || continue
    candidate_state=$(resolve_tree "$candidate") || continue
    state_path_exists "$from_state" "$candidate_state" || continue
    git diff --quiet "$candidate" "$head" -- '*.rs' || continue
    printf '%s\n' "$candidate"
    return 0
  done
  return 1
}

print_checkout_hint() {
  head=$1
  local_ref=${2:-}
  current_head=$(resolve_commit HEAD) || return 0
  if test "$head" = "$current_head"; then
    return 0
  fi
  if test -n "$local_ref"; then
    echo "First check out $local_ref at $head." >&2
  else
    echo "First check out the ref at $head." >&2
  fi
}

check_receipts() {
  test "$#" -ge 2 && test "$#" -le 3 || usage
  from_ref=$1
  to_ref=$2
  local_ref=${3:-}

  to=$(resolve_commit "$to_ref") || die "invalid local head: $to_ref"
  remote_name=${PRE_COMMIT_REMOTE_NAME:-origin}
  if is_zero_oid "$from_ref"; then
    from=$(find_new_branch_base "$remote_name" "$to")
  else
    from=$(git rev-parse --verify "$from_ref" 2>/dev/null) || {
      echo "Goalrail verification receipt: remote base $from_ref is missing locally." >&2
      echo "Fetch it before retrying: git fetch --prune $remote_name" >&2
      exit 1
    }
  fi

  from_state=$(resolve_tree "$from") || die "invalid remote tree: $from"
  to_state=$(resolve_tree "$to") || die "invalid local tree: $to"
  if test "$from_state" = "$to_state"; then
    echo "Goalrail verification receipts: no new tree state for $local_ref"
    return 0
  fi

  state_covered=false
  if state_path_exists "$from_state" "$to_state"; then
    state_covered=true
  fi
  ci_covered=false
  if has_ci_receipt "$to_state"; then
    ci_covered=true
  fi

  if test "$state_covered" = true && test "$ci_covered" = true; then
    echo "Goalrail verification receipts cover ref tip $from..$to"
    return 0
  fi

  echo "Goalrail verification receipt: evidence is missing for ref tip $from..$to" >&2
  print_checkout_hint "$to" "$local_ref"
  echo "Run this before retrying the push:" >&2
  if ci_base=$(find_ci_recovery_base "$from_state" "$to"); then
    echo "  mise run verify:ci-state -- $ci_base $to" >&2
  else
    echo "  mise run verify:outgoing-rust -- $from $to" >&2
  fi
  exit 1
}

verify_outgoing() {
  test "$#" -ge 1 && test "$#" -le 2 || usage
  base=$(git rev-parse --verify "$1" 2>/dev/null) ||
    die "invalid outgoing base: $1"
  resolve_tree "$base" >/dev/null || die "outgoing base has no tree: $base"
  head_ref=${2:-HEAD}
  head=$(resolve_commit "$head_ref") || die "invalid outgoing head: $head_ref"
  current_head=$(resolve_commit HEAD) || die "cannot resolve HEAD"
  test "$head" = "$current_head" ||
    die "outgoing verification must target the checked-out HEAD: $current_head"

  committed_tree=$(resolve_tree "$head")
  before_tree=$(tree_hash_for_worktree)
  test "$committed_tree" = "$before_tree" ||
    die "working tree changes affect the verification tree; commit or isolate them first"

  diff_path="$temp_root/outgoing-rust.diff"
  git diff --binary "$base" "$head" >"$diff_path"
  test -s "$diff_path" || die "outgoing range is empty: $base..$head"

  mise run ci
  if git diff --quiet "$base" "$head" -- '*.rs'; then
    echo "Mutation: NOT_APPLICABLE - outgoing range changes no Rust source files"
  else
    GOALRAIL_MUTATION_DIFF="$diff_path" mise run mutants:diff
  fi
  after_tree=$(tree_hash_for_worktree)
  test "$before_tree" = "$after_tree" ||
    die "working tree changed during outgoing verification; no receipt recorded"
  record_milestone_receipt "$base" "$diff_path"
}

command=${1:-}
test -n "$command" || usage
shift

case "$command" in
  verify-milestone) verify_current_milestone "$@" ;;
  verify-ci) verify_ci_state "$@" ;;
  check) check_receipts "$@" ;;
  verify-outgoing) verify_outgoing "$@" ;;
  *) usage ;;
esac

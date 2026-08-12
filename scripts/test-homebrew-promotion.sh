#!/bin/sh

set -eu

fail() {
  echo "homebrew-promotion-test: $1" >&2
  exit 1
}

checksum() {
  file=$1
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  fi
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
subject="$repo_root/scripts/promote-homebrew.sh"

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
[ -x "$subject" ] || fail "Homebrew promotion script is not executable"
sh -n "$subject"
if grep -Eq '(^|[[:space:]])brew([[:space:]]|$)' "$subject"; then
  fail "promotion script must not interpret or mutate Homebrew"
fi

fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-homebrew-promotion-test.XXXXXX")
cleanup() {
  rm -rf "$fixture_root"
}
trap cleanup EXIT HUP INT TERM

source_root="$fixture_root/source"
mkdir -p "$source_root/scripts" "$source_root/dist/v1.2.3"
cp "$subject" "$source_root/scripts/promote-homebrew.sh"
cat >"$source_root/scripts/check-github-run.sh" <<'EOF'
#!/bin/sh
printf '{"schemaVersion":1,"verdict":"RUN_VERIFIED","runAttempt":"1"}\n'
EOF
for checker in check-release-bundle.sh check-github-release.sh; do
  cat >"$source_root/scripts/$checker" <<'EOF'
#!/bin/sh
exit 0
EOF
done
chmod 0755 "$source_root/scripts/"*.sh
printf 'dist/\n' >"$source_root/.gitignore"
printf 'fixture source\n' >"$source_root/README.md"
git -C "$source_root" init -q -b main
git -C "$source_root" config user.name 'Fixture Owner'
git -C "$source_root" config user.email '12345+fixture@users.noreply.github.com'
git -C "$source_root" add .
git -C "$source_root" commit -q -m 'Fixture release source'
source_commit=$(git -C "$source_root" rev-parse HEAD)
git -C "$source_root" tag -a v1.2.3 -m 'Fixture v1.2.3'
git init -q --bare "$fixture_root/source-origin.git"
git -C "$source_root" remote add origin "$fixture_root/source-origin.git"
git -C "$source_root" push -q origin main refs/tags/v1.2.3

archive="$source_root/dist/v1.2.3/goalrail-v1.2.3-aarch64-apple-darwin.tar.gz"
printf 'verified public archive bytes\n' >"$archive"
archive_sha=$(checksum "$archive")
formula="$source_root/dist/v1.2.3/goalrail.rb"
cat >"$formula" <<EOF
class Goalrail < Formula
  url "https://github.com/heurema/goalrail-rs/releases/download/v1.2.3/goalrail-v1.2.3-aarch64-apple-darwin.tar.gz"
  sha256 "$archive_sha"
end
EOF
jq -n --arg source "$source_commit" '{
  version: "1.2.3",
  sourceCommit: $source,
  run: {id: "123", attempt: "1"}
}' >"$source_root/dist/v1.2.3/release.json"

fake_bin="$fixture_root/bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/gh" <<'EOF'
#!/bin/sh
if [ "$#" -eq 4 ] && [ "$1" = api ] &&
  [ "$2" = repos/heurema/homebrew-tap ] && [ "$3" = --jq ] &&
  [ "$4" = .permissions.push ]; then
  printf '%s\n' "${GOALRAIL_TEST_PUSH_PERMISSION:-true}"
  exit 0
fi
echo "unexpected gh arguments: $*" >&2
exit 90
EOF
cat >"$fake_bin/curl" <<'EOF'
#!/bin/sh
output=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      output=$2
      shift 2
      ;;
    *) shift ;;
  esac
done
[ -n "$output" ] || exit 91
cp "${GOALRAIL_TEST_ARCHIVE:?}" "$output"
EOF
real_git=$(command -v git)
cat >"$fake_bin/git" <<'EOF'
#!/bin/sh

set -eu

real_git=${GOALRAIL_TEST_REAL_GIT:?}
if [ "${GOALRAIL_TEST_DRY_RUN_FAIL:-false}" = true ]; then
  for argument in "$@"; do
    [ "$argument" != --dry-run ] || exit 86
  done
fi
if [ -n "${GOALRAIL_TEST_RACE_REMOTE:-}" ] &&
  [ "$#" -ge 4 ] && [ "$1" = ls-remote ] && [ "$2" = --exit-code ] &&
  [ "$3" = git@github.com:heurema/homebrew-tap.git ] &&
  [ "$4" = refs/heads/main ]; then
  counter=${GOALRAIL_TEST_RACE_COUNTER:?}
  count=0
  [ ! -f "$counter" ] || count=$(cat "$counter")
  count=$((count + 1))
  printf '%s\n' "$count" >"$counter"
  if [ "$count" -eq 2 ]; then
    remote=${GOALRAIL_TEST_RACE_REMOTE:?}
    old=$($real_git --git-dir="$remote" rev-parse refs/heads/main)
    tree=$($real_git --git-dir="$remote" rev-parse "$old^{tree}")
    raced=$(printf 'Concurrent tap update\n' |
      GIT_AUTHOR_NAME=fixture GIT_AUTHOR_EMAIL=fixture@example.invalid \
      GIT_COMMITTER_NAME=fixture GIT_COMMITTER_EMAIL=fixture@example.invalid \
      $real_git --git-dir="$remote" commit-tree "$tree" -p "$old")
    $real_git --git-dir="$remote" update-ref refs/heads/main "$raced" "$old"
  fi
fi
exec "$real_git" "$@"
EOF
chmod 0755 "$fake_bin/gh" "$fake_bin/curl" "$fake_bin/git"

seed_tap() {
  remote=$1
  work=$2
  checkout=$3
  mkdir -p "$work/Formula"
  cat >"$work/Formula/goalrail.rb" <<'EOF'
class Goalrail < Formula
  url "https://github.com/heurema/goalrail-rs/releases/download/v1.2.2/goalrail-v1.2.2-aarch64-apple-darwin.tar.gz"
  sha256 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
end
EOF
  git -C "$work" init -q -b main
  git -C "$work" config user.name fixture
  git -C "$work" config user.email fixture@example.invalid
  git -C "$work" add Formula/goalrail.rb
  git -C "$work" commit -q -m 'Seed Goalrail v1.2.2'
  git init -q --bare "$remote"
  git --git-dir="$remote" symbolic-ref HEAD refs/heads/main
  git -C "$work" remote add origin "$remote"
  git -C "$work" push -q origin main
  git clone -q --branch main "$remote" "$checkout"
  git -C "$checkout" config remote.origin.url \
    https://github.com/heurema/homebrew-tap.git
}

run_promotion() {
  remote=$1
  tap=$2
  permission=${3:-true}
  race_remote=${4:-}
  dry_run_fail=${5:-false}
  remote_url="file://$remote"
  GOALRAIL_TEST_ARCHIVE="$archive" \
  GOALRAIL_TEST_PUSH_PERMISSION="$permission" \
  GOALRAIL_TEST_REAL_GIT="$real_git" \
  GOALRAIL_TEST_RACE_REMOTE="$race_remote" \
  GOALRAIL_TEST_RACE_COUNTER="$fixture_root/race-counter" \
  GOALRAIL_TEST_DRY_RUN_FAIL="$dry_run_fail" \
  PATH="$fake_bin:$PATH" \
  GIT_CONFIG_COUNT=3 \
  GIT_CONFIG_KEY_0=url."$remote_url".insteadOf \
  GIT_CONFIG_VALUE_0=git@github.com:heurema/homebrew-tap.git \
  GIT_CONFIG_KEY_1=url."$remote_url".insteadOf \
  GIT_CONFIG_VALUE_1=https://github.com/heurema/homebrew-tap.git \
  GIT_CONFIG_KEY_2=protocol.file.allow \
  GIT_CONFIG_VALUE_2=always \
    "$source_root/scripts/promote-homebrew.sh" 1.2.3 123 1 "$tap"
}

tap_remote="$fixture_root/tap.git"
tap_seed="$fixture_root/tap-seed"
tap_checkout="$fixture_root/tap-checkout"
seed_tap "$tap_remote" "$tap_seed" "$tap_checkout"

published=$(run_promotion "$tap_remote" "$tap_checkout") ||
  fail "verified promotion unexpectedly failed"
printf '%s\n' "$published" | jq -e '
  .schemaVersion == 1 and .verdict == "PUBLISHED" and .version == "1.2.3"
' >/dev/null || fail "promotion did not return PUBLISHED"
git --git-dir="$tap_remote" show main:Formula/goalrail.rb >"$fixture_root/published.rb"
cmp -s "$formula" "$fixture_root/published.rb" ||
  fail "remote formula bytes differ after promotion"

no_change=$(run_promotion "$tap_remote" "$tap_checkout") ||
  fail "byte-identical rerun unexpectedly failed"
printf '%s\n' "$no_change" | jq -e '.verdict == "NO_CHANGE"' >/dev/null ||
  fail "byte-identical rerun did not return NO_CHANGE"

conflict_work="$fixture_root/conflict-work"
git clone -q --branch main "$tap_remote" "$conflict_work"
sed "s/$archive_sha/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/" \
  "$formula" >"$conflict_work/Formula/goalrail.rb"
git -C "$conflict_work" config user.name fixture
git -C "$conflict_work" config user.email fixture@example.invalid
git -C "$conflict_work" add Formula/goalrail.rb
git -C "$conflict_work" commit -q -m 'Conflicting Goalrail v1.2.3'
git -C "$conflict_work" push -q origin main
if same_version=$(run_promotion "$tap_remote" "$tap_checkout"); then
  fail "same-version byte conflict unexpectedly succeeded"
else
  same_version_status=$?
fi
[ "$same_version_status" -eq 4 ] || fail "same-version conflict returned the wrong status"
printf '%s\n' "$same_version" | jq -e '
  .verdict == "CONFLICT" and .finding.code == "SAME_VERSION_CONFLICT"
' >/dev/null || fail "same-version conflict was not classified"

retry_remote="$fixture_root/retry-tap.git"
retry_seed="$fixture_root/retry-seed"
retry_checkout="$fixture_root/retry-checkout"
seed_tap "$retry_remote" "$retry_seed" "$retry_checkout"
rejection_log="$fixture_root/rejection.log"
cat >"$retry_remote/hooks/pre-receive" <<EOF
#!/bin/sh
printf 'rejected\n' >>"$rejection_log"
exit 1
EOF
chmod 0755 "$retry_remote/hooks/pre-receive"
retry_remote_before=$(git --git-dir="$retry_remote" rev-parse main)
if rejected=$(run_promotion "$retry_remote" "$retry_checkout"); then
  fail "server-rejected push unexpectedly succeeded"
else
  rejected_status=$?
fi
[ "$rejected_status" -eq 4 ] || fail "rejected push returned the wrong status"
printf '%s\n' "$rejected" | jq -e '
  .verdict == "CONFLICT" and .finding.code == "PUSH_REJECTED"
' >/dev/null || fail "rejected push was not classified"
[ "$(wc -l <"$rejection_log" | tr -d ' ')" -eq 1 ] ||
  fail "rejected push was retried"
[ "$(git --git-dir="$retry_remote" rev-parse main)" = "$retry_remote_before" ] ||
  fail "rejected push changed the remote"
[ "$(git -C "$retry_checkout" rev-parse HEAD^)" = "$retry_remote_before" ] ||
  fail "rejected push did not retain one exact resumable commit"
[ -z "$(git -C "$retry_checkout" status --porcelain)" ] ||
  fail "rejected push left a dirty tap"

rm "$retry_remote/hooks/pre-receive"
resumed=$(run_promotion "$retry_remote" "$retry_checkout") ||
  fail "exact interrupted commit did not resume"
printf '%s\n' "$resumed" | jq -e '.verdict == "PUBLISHED"' >/dev/null ||
  fail "exact interrupted commit returned the wrong verdict"

permission_remote="$fixture_root/permission-tap.git"
permission_seed="$fixture_root/permission-seed"
permission_checkout="$fixture_root/permission-checkout"
seed_tap "$permission_remote" "$permission_seed" "$permission_checkout"
permission_head=$(git -C "$permission_checkout" rev-parse HEAD)
if denied=$(run_promotion "$permission_remote" "$permission_checkout" false); then
  fail "missing GitHub write permission unexpectedly succeeded"
else
  denied_status=$?
fi
[ "$denied_status" -eq 4 ] || fail "permission failure returned the wrong status"
printf '%s\n' "$denied" | jq -e '
  .verdict == "CONFLICT" and .finding.code == "WRITE_PERMISSION_MISSING"
' >/dev/null || fail "permission failure was not classified"
[ "$(git -C "$permission_checkout" rev-parse HEAD)" = "$permission_head" ] &&
  [ -z "$(git -C "$permission_checkout" status --porcelain)" ] ||
  fail "permission failure mutated the tap"

transport_remote="$fixture_root/transport-tap.git"
transport_seed="$fixture_root/transport-seed"
transport_checkout="$fixture_root/transport-checkout"
seed_tap "$transport_remote" "$transport_seed" "$transport_checkout"
transport_head=$(git -C "$transport_checkout" rev-parse HEAD)
if transport_failed=$(run_promotion \
  "$transport_remote" "$transport_checkout" true "" true); then
  fail "failed dry-run push transport unexpectedly succeeded"
else
  transport_status=$?
fi
[ "$transport_status" -eq 4 ] ||
  fail "dry-run transport failure returned the wrong status"
printf '%s\n' "$transport_failed" | jq -e '
  .verdict == "CONFLICT" and .finding.code == "PUSH_PREFLIGHT_FAILED"
' >/dev/null || fail "dry-run transport failure was not classified"
[ "$(git -C "$transport_checkout" rev-parse HEAD)" = "$transport_head" ] &&
  [ -z "$(git -C "$transport_checkout" status --porcelain)" ] ||
  fail "dry-run transport failure mutated the tap"

race_remote="$fixture_root/race-tap.git"
race_seed="$fixture_root/race-seed"
race_checkout="$fixture_root/race-checkout"
seed_tap "$race_remote" "$race_seed" "$race_checkout"
rm -f "$fixture_root/race-counter"
race_base=$(git --git-dir="$race_remote" rev-parse main)
if raced=$(run_promotion "$race_remote" "$race_checkout" true "$race_remote"); then
  fail "concurrent remote update unexpectedly succeeded"
else
  race_status=$?
fi
[ "$race_status" -eq 4 ] || fail "remote race returned the wrong status"
printf '%s\n' "$raced" | jq -e '
  .verdict == "CONFLICT" and .finding.code == "REMOTE_RACE"
' >/dev/null || fail "remote race was not classified"
race_head=$(git --git-dir="$race_remote" rev-parse main)
[ "$race_head" != "$race_base" ] || fail "remote race fixture did not advance main"
[ "$(git -C "$race_checkout" rev-parse HEAD^)" = "$race_base" ] ||
  fail "remote race did not retain the exact prepared commit"
[ "$(git -C "$race_checkout" rev-parse HEAD)" != "$race_head" ] ||
  fail "remote race pushed the prepared commit"
[ -z "$(git -C "$race_checkout" status --porcelain)" ] ||
  fail "remote race left a dirty tap"

echo "HOMEBREW_PROMOTION_TEST_OK published=verified no_change=byte-exact conflict=same-version rejected_push=single-attempt resume=exact permission=pre-mutation transport=pre-mutation race=no-push"

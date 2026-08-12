#!/bin/sh

set -eu

fail() {
  echo "plugin-update-target-test: $1" >&2
  exit 1
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
subject="$repo_root/plugins/goalrail/skills/goalrail/scripts/plugin-update-target.sh"
real_git=$(command -v git)

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
[ -x "$subject" ] || fail "bundled plugin target diagnostic is not executable"
sh -n "$subject"

fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-plugin-target-test.XXXXXX")
cleanup() {
  rm -rf "$fixture_root"
}
trap cleanup EXIT HUP INT TERM

remote_commit=1111111111111111111111111111111111111111
plugin_root="$fixture_root/plugin"
fake_bin="$fixture_root/bin"
mkdir -p \
  "$plugin_root/.codex-plugin" \
  "$plugin_root/skills/goalrail/references" \
  "$plugin_root/skills/goalrail/scripts" \
  "$fake_bin"

cat >"$fixture_root/marketplace.json" <<'EOF'
{"name":"goalrail","plugins":[{"name":"goalrail","source":{"source":"git-subdir","url":"https://github.com/heurema/goalrail-rs.git","path":"./plugins/goalrail","ref":"v1.2.3"}}]}
EOF
cat >"$plugin_root/.codex-plugin/plugin.json" <<'EOF'
{"name":"goalrail","version":"1.2.3","skills":"./skills/"}
EOF
cat >"$plugin_root/skills/goalrail/SKILL.md" <<'EOF'
---
name: goalrail
description: Fixture Goalrail skill.
---
EOF
for file in index.md install.md plugin-lifecycle.md update-discovery.md; do
  printf '%s\n' "$file fixture" >"$plugin_root/skills/goalrail/references/$file"
done
printf '%s\n' '#!/bin/sh' 'exit 0' >"$plugin_root/skills/goalrail/scripts/homebrew-update-state.sh"
printf '%s\n' '#!/bin/sh' 'exit 0' >"$plugin_root/skills/goalrail/scripts/plugin-update-target.sh"
chmod 0755 \
  "$plugin_root/skills/goalrail/scripts/homebrew-update-state.sh" \
  "$plugin_root/skills/goalrail/scripts/plugin-update-target.sh"

make_tree() {
  omit=$1
  find "$plugin_root" -type f | LC_ALL=C sort | while IFS= read -r file; do
    relative=${file#"$plugin_root/"}
    [ "$relative" = "$omit" ] && continue
    mode=100644
    [ -x "$file" ] && mode=100755
    sha=$($real_git hash-object "$file")
    jq -cn --arg path "plugins/goalrail/$relative" --arg mode "$mode" --arg sha "$sha" \
      '{path:$path,mode:$mode,type:"blob",sha:$sha}'
  done | jq -sc '{sha:"fixture",url:"fixture",tree:.,truncated:false}'
}
make_tree none >"$fixture_root/tree.json"
make_tree skills/goalrail/SKILL.md >"$fixture_root/tree-missing-skill.json"

cat >"$fake_bin/curl" <<'EOF'
#!/bin/sh

set -eu

output=
url=
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      output=$2
      shift 2
      ;;
    --fail|--silent|--show-error|--location)
      shift
      ;;
    *)
      url=$1
      shift
      ;;
  esac
done
[ -n "$output" ] && [ -n "$url" ] || exit 90
root=${GOALRAIL_PLUGIN_FIXTURE_ROOT:?}
scenario=${GOALRAIL_PLUGIN_TARGET_SCENARIO:-ok}
case "$url" in
  */.agents/plugins/marketplace.json)
    cp "$root/marketplace.json" "$output"
    ;;
  */plugins/goalrail/.codex-plugin/plugin.json)
    if [ "$scenario" = manifest-version ]; then
      printf '%s\n' '{"name":"goalrail","version":"9.9.9","skills":"./skills/"}' >"$output"
    elif [ "$scenario" = manifest-tree ]; then
      printf '%s\n' '{"name":"goalrail","version":"1.2.3","skills":"./skills/","unexpected":true}' >"$output"
    else
      cp "$root/plugin/.codex-plugin/plugin.json" "$output"
    fi
    ;;
  */plugins/goalrail/skills/goalrail/SKILL.md)
    if [ "$scenario" = skill-tree ]; then
      printf '%s\n' '---' 'name: goalrail' 'description: Different valid skill.' '---' >"$output"
    else
      cp "$root/plugin/skills/goalrail/SKILL.md" "$output"
    fi
    ;;
  *'/git/trees/'*)
    if [ "$scenario" = missing-skill ]; then
      cp "$root/tree-missing-skill.json" "$output"
    else
      cp "$root/tree.json" "$output"
    fi
    ;;
  *)
    exit 91
    ;;
esac
EOF
chmod 0755 "$fake_bin/curl"

cat >"$fake_bin/git" <<'EOF'
#!/bin/sh

set -eu

case "$1" in
  hash-object)
    exec "${GOALRAIL_REAL_GIT:?}" "$@"
    ;;
  ls-remote)
    printf '%s\t%s\n' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa refs/tags/v1.2.3
    printf '%s\t%s\n' "${GOALRAIL_REMOTE_COMMIT:?}" 'refs/tags/v1.2.3^{}'
    ;;
  *)
    exit 92
    ;;
esac
EOF
chmod 0755 "$fake_bin/git"

run_subject() {
  scenario=$1
  shift
  GOALRAIL_PLUGIN_FIXTURE_ROOT="$fixture_root" \
    GOALRAIL_PLUGIN_TARGET_SCENARIO="$scenario" \
    GOALRAIL_REAL_GIT="$real_git" \
    GOALRAIL_REMOTE_COMMIT="$remote_commit" \
    PATH="$fake_bin:$PATH" \
    "$subject" "$remote_commit" 1.2.3 "$@"
}

success=$(run_subject ok "$plugin_root") || fail "valid target unexpectedly failed"
printf '%s\n' "$success" | jq -e '
  .verdict == "TARGET_VERIFIED"
  and .manager == "CODEX_PLUGIN"
  and .remoteCandidate.version == "1.2.3"
  and .remoteCandidate.ref == "v1.2.3"
  and .remoteCandidate.commit == "1111111111111111111111111111111111111111"
  and .remoteCandidate.payloadFiles == 8
  and .remoteCandidate.channelLag == false
  and .cacheVerified == true
' >/dev/null || fail "valid target receipt is incomplete"

run_blocked() {
  scenario=$1
  expected=$2
  shift 2
  if output=$(run_subject "$scenario" "$@"); then
    fail "$scenario unexpectedly succeeded"
  else
    status=$?
  fi
  [ "$status" -eq 4 ] || fail "$scenario returned exit $status instead of 4"
  printf '%s\n' "$output" | jq -e --arg code "$expected" '
    .verdict == "BLOCKED" and .manager == "CODEX_PLUGIN" and .finding.code == $code
  ' >/dev/null || fail "$scenario omitted $expected"
}

run_blocked missing-skill PAYLOAD_INCOMPLETE
run_blocked manifest-version MANIFEST_INVALID
run_blocked manifest-tree MANIFEST_TREE_MISMATCH
run_blocked skill-tree SKILL_TREE_MISMATCH

chmod 0755 "$plugin_root/skills/goalrail/references/index.md"
run_blocked ok CACHE_MODE_MISMATCH "$plugin_root"
chmod 0644 "$plugin_root/skills/goalrail/references/index.md"

mv "$plugin_root/skills/goalrail/references/index.md" "$fixture_root/index.md"
ln -s "$fixture_root/index.md" "$plugin_root/skills/goalrail/references/index.md"
run_blocked ok CACHE_SYMLINK "$plugin_root"
rm "$plugin_root/skills/goalrail/references/index.md"
mv "$fixture_root/index.md" "$plugin_root/skills/goalrail/references/index.md"

ln -s "$fixture_root/marketplace.json" "$plugin_root/extra-link"
run_blocked ok CACHE_SYMLINK "$plugin_root"
rm "$plugin_root/extra-link"

ln -s "$plugin_root" "$fixture_root/plugin-link"
run_blocked ok CACHE_SYMLINK_ROOT "$fixture_root/plugin-link"
rm "$fixture_root/plugin-link"

mkfifo "$plugin_root/extra-fifo"
run_blocked ok CACHE_ENTRY_INVALID "$plugin_root"
rm "$plugin_root/extra-fifo"

printf '%s\n' 'changed' >>"$plugin_root/skills/goalrail/references/index.md"
run_blocked ok CACHE_BLOB_MISMATCH "$plugin_root"

echo "PLUGIN_UPDATE_TARGET_TEST_OK scenarios=11 tree=exact cache=blob-mode-entry-matched"

#!/bin/sh

set -eu

fail() {
  echo "goalrail remote plugin smoke: $1" >&2
  exit 1
}

[ "$#" -eq 2 ] || fail "usage: $0 <git-ref> <plugin-version>"

remote_ref=$1
expected_version=$2
remote_source=heurema/goalrail-rs
payload_url=https://github.com/heurema/goalrail-rs.git
payload_path=./plugins/goalrail
payload_ref=v$expected_version

[ -n "$remote_ref" ] || fail "git ref must not be empty"
printf '%s\n' "$expected_version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' ||
  fail "release version must be semantic x.y.z"

command -v codex >/dev/null 2>&1 || fail "codex is unavailable"
command -v git >/dev/null 2>&1 || fail "git is unavailable"
command -v jq >/dev/null 2>&1 || fail "jq is unavailable"

trial_codex_home=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-plugin-remote.XXXXXX")
app_server_pid=

cleanup() {
  if [ -n "$app_server_pid" ] && kill -0 "$app_server_pid" 2>/dev/null; then
    kill "$app_server_pid" 2>/dev/null || true
    wait "$app_server_pid" 2>/dev/null || true
  fi
  rm -rf "$trial_codex_home"
}
trap cleanup EXIT HUP INT TERM

export CODEX_HOME=$trial_codex_home

marketplace_result=$(codex plugin marketplace add "$remote_source" --ref "$remote_ref" --json)
printf '%s\n' "$marketplace_result" |
  jq -e '
    .marketplaceName == "goalrail"
    and .alreadyAdded == false
  ' >/dev/null
marketplace_root=$(printf '%s\n' "$marketplace_result" | jq -er '.installedRoot')
marketplace_manifest="$marketplace_root/.agents/plugins/marketplace.json"
test -s "$marketplace_manifest" || fail "remote marketplace manifest is missing"

jq -e \
  --arg url "$payload_url" \
  --arg path "$payload_path" \
  --arg ref "$payload_ref" '
    any(.plugins[];
      .name == "goalrail"
      and .source.source == "git-subdir"
      and .source.url == $url
      and .source.path == $path
      and .source.ref == $ref
    )
  ' "$marketplace_manifest" >/dev/null ||
  fail "remote marketplace advertises an unexpected Goalrail payload"

if ! payload_tag_lines=$(git ls-remote --exit-code "$payload_url" \
  "refs/tags/$payload_ref" "refs/tags/$payload_ref^{}"); then
  fail "remote plugin tag is unavailable: $payload_ref"
fi
payload_sha=$(printf '%s\n' "$payload_tag_lines" | awk '
  /\^\{\}$/ { peeled = $1 }
  !/\^\{\}$/ { direct = $1 }
  END {
    if (peeled != "") print peeled
    else if (direct != "") print direct
  }
')
printf '%s\n' "$payload_sha" | grep -Eq '^[0-9a-f]{40}$' ||
  fail "remote plugin tag did not resolve to one commit"

codex plugin marketplace list --json |
  jq -e '
    any(.marketplaces[];
      .name == "goalrail"
      and .marketplaceSource.sourceType == "git"
    )
  ' >/dev/null

install_result=$(codex plugin add goalrail@goalrail --json)
printf '%s\n' "$install_result" |
  jq -e --arg version "$expected_version" '
    .pluginId == "goalrail@goalrail"
    and .version == $version
  ' >/dev/null
installed_path=$(printf '%s\n' "$install_result" | jq -er '.installedPath')
installed_skill="$installed_path/skills/goalrail/SKILL.md"
test -s "$installed_skill" || fail "installed Goalrail skill is missing"

codex plugin list --json |
  jq -e --arg version "$expected_version" '
    any(.installed[];
      .pluginId == "goalrail@goalrail"
      and .installed == true
      and .enabled == true
      and .version == $version
      and .marketplaceSource.sourceType == "git"
    )
  ' >/dev/null

rpc_requests="$trial_codex_home/app-server.requests"
rpc_responses="$trial_codex_home/app-server.responses"
rpc_errors="$trial_codex_home/app-server.errors"
mkfifo "$rpc_requests"
: >"$rpc_responses"

codex app-server --stdio \
  <"$rpc_requests" >"$rpc_responses" 2>"$rpc_errors" &
app_server_pid=$!
exec 3>"$rpc_requests"

wait_for_rpc_id() {
  rpc_id=$1
  attempts=0
  while [ "$attempts" -lt 100 ]; do
    if jq -e --argjson id "$rpc_id" 'select(.id == $id)' \
      "$rpc_responses" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "$app_server_pid" 2>/dev/null; then
      fail "Codex app-server exited before RPC response $rpc_id"
    fi
    attempts=$((attempts + 1))
    sleep 0.1
  done
  fail "Codex app-server timed out before RPC response $rpc_id"
}

jq -cn --arg version "$expected_version" '{
  id: 1,
  method: "initialize",
  params: {
    clientInfo: {name: "goalrail-plugin-smoke", version: $version},
    capabilities: {experimentalApi: true}
  }
}' >&3
wait_for_rpc_id 1

jq -cn --arg cwd "$PWD" '{
  id: 2,
  method: "skills/list",
  params: {cwds: [$cwd], forceReload: true}
}' >&3
wait_for_rpc_id 2

jq -e --arg skill "$installed_skill" '
  select(.id == 2)
  | any(.result.data[];
      (.errors | length) == 0
      and any(.skills[];
        .name == "goalrail:goalrail"
        and .path == $skill
        and .enabled == true
      )
    )
' "$rpc_responses" >/dev/null ||
  fail "skills/list did not expose the installed Goalrail skill"

exec 3>&-
if kill -0 "$app_server_pid" 2>/dev/null; then
  kill "$app_server_pid" 2>/dev/null || true
fi
wait "$app_server_pid" 2>/dev/null || true
app_server_pid=

printf '%s\n' \
  "GOALRAIL_REMOTE_PLUGIN_SMOKE_OK source=$remote_source ref=$remote_ref payload_ref=$payload_ref payload_sha=$payload_sha version=$expected_version skill=goalrail:goalrail"

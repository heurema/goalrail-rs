#!/bin/sh

set -u

manager=CODEX_PLUGIN
repository=https://github.com/heurema/goalrail-rs.git

blocked() {
  code=$1
  message=$2
  jq -cn --arg manager "$manager" --arg code "$code" --arg message "$message" '{
    schemaVersion: 1,
    verdict: "BLOCKED",
    manager: $manager,
    finding: { code: $code, message: $message }
  }'
  exit 4
}

command -v jq >/dev/null 2>&1 || {
  printf '%s\n' '{"schemaVersion":1,"verdict":"BLOCKED","manager":"CODEX_PLUGIN","finding":{"code":"JQ_MISSING","message":"jq is unavailable"}}'
  exit 4
}
command -v curl >/dev/null 2>&1 || blocked "CURL_MISSING" "curl is unavailable"
command -v git >/dev/null 2>&1 || blocked "GIT_MISSING" "git is unavailable"
command -v find >/dev/null 2>&1 || blocked "FIND_MISSING" "find is unavailable"

[ "$#" -eq 2 ] || [ "$#" -eq 3 ] ||
  blocked "ARGUMENTS_INVALID" "expected remote commit, latest release, and optional cache root"
remote_commit=$1
latest_release=$2
cache_root=${3-}

printf '%s\n' "$remote_commit" |
  grep -Eq '^[0-9a-f]{40}$' || blocked "COMMIT_INVALID" "remote commit must be a full lowercase SHA"
printf '%s\n' "$latest_release" |
  grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' ||
  blocked "VERSION_INVALID" "latest release must be a stable semantic version"

probe_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-plugin-target.XXXXXX") ||
  blocked "TEMP_UNAVAILABLE" "temporary probe storage could not be created"
marketplace_file="$probe_root/marketplace.json"
manifest_file="$probe_root/plugin.json"
skill_file="$probe_root/SKILL.md"
tree_file="$probe_root/tree.json"
payload_file="$probe_root/payload.json"
payload_tsv="$probe_root/payload.tsv"
cleanup() {
  rm -f "$marketplace_file" "$manifest_file" "$skill_file" "$tree_file" \
    "$payload_file" "$payload_tsv"
  rmdir "$probe_root" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

raw_base="https://raw.githubusercontent.com/heurema/goalrail-rs/$remote_commit"
curl --fail --silent --show-error --location \
  --output "$marketplace_file" "$raw_base/.agents/plugins/marketplace.json" ||
  blocked "MARKETPLACE_FETCH_FAILED" "exact remote marketplace could not be read"
curl --fail --silent --show-error --location \
  --output "$manifest_file" "$raw_base/plugins/goalrail/.codex-plugin/plugin.json" ||
  blocked "MANIFEST_FETCH_FAILED" "exact remote plugin manifest could not be read"
curl --fail --silent --show-error --location \
  --output "$skill_file" "$raw_base/plugins/goalrail/skills/goalrail/SKILL.md" ||
  blocked "SKILL_FETCH_FAILED" "exact remote Goalrail skill could not be read"
curl --fail --silent --show-error --location \
  --output "$tree_file" \
  "https://api.github.com/repos/heurema/goalrail-rs/git/trees/$remote_commit?recursive=1" ||
  blocked "TREE_FETCH_FAILED" "exact remote Git tree could not be read"

candidate=$(jq -ec '
  [.plugins[]? | select(.name == "goalrail")]
  | if length == 1 then .[0] else error("ambiguous Goalrail entry") end
  | select(.source == {
      source: "git-subdir",
      url: "https://github.com/heurema/goalrail-rs.git",
      path: "./plugins/goalrail",
      ref: .source.ref
    })
' "$marketplace_file" 2>/dev/null) ||
  blocked "MARKETPLACE_INVALID" "remote marketplace has an invalid Goalrail entry"
candidate_ref=$(printf '%s\n' "$candidate" | jq -er '.source.ref') ||
  blocked "MARKETPLACE_INVALID" "remote marketplace omitted the Goalrail payload ref"
printf '%s\n' "$candidate_ref" |
  grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$' ||
  blocked "VERSION_INVALID" "remote marketplace has a non-stable Goalrail ref"
candidate_version=${candidate_ref#v}

comparison=$(jq -nr --arg candidate "$candidate_version" --arg latest "$latest_release" '
  ($candidate | split(".") | map(tonumber)) as $candidate_parts
  | ($latest | split(".") | map(tonumber)) as $latest_parts
  | if $candidate_parts == $latest_parts then "equal"
    elif $candidate_parts < $latest_parts then "lower"
    else "higher"
    end
') || blocked "VERSION_INVALID" "Goalrail versions could not be compared"
[ "$comparison" != higher ] ||
  blocked "CHANNEL_AHEAD" "remote marketplace is newer than the latest public release"

jq -e --arg version "$candidate_version" '
  .name == "goalrail"
  and .version == $version
  and .skills == "./skills/"
' "$manifest_file" >/dev/null 2>&1 ||
  blocked "MANIFEST_INVALID" "remote plugin manifest does not match the advertised version"
sed -n '1,8p' "$skill_file" | grep -F 'name: goalrail' >/dev/null 2>&1 ||
  blocked "SKILL_INVALID" "remote Goalrail SKILL.md has an invalid identity"
sed -n '1,8p' "$skill_file" | grep -F 'description:' >/dev/null 2>&1 ||
  blocked "SKILL_INVALID" "remote Goalrail SKILL.md has no description"

jq -e '
  .truncated == false
  and (.tree | type == "array")
  and all(
    .tree[]
    | select(.path | startswith("plugins/goalrail/"))
    | select(.type != "tree");
    .type == "blob"
    and (.mode == "100644" or .mode == "100755")
    and (.path | test("^plugins/goalrail/[A-Za-z0-9._/-]+$"))
    and (.path | contains("//") | not)
    and (.path | contains("/./") | not)
    and (.path | contains("/../") | not)
  )
' "$tree_file" >/dev/null 2>&1 ||
  blocked "TREE_INVALID" "remote plugin tree is truncated or contains unsafe entries"

jq -c '[
  .tree[]
  | select(.type == "blob" and (.path | startswith("plugins/goalrail/")))
  | {path: (.path | ltrimstr("plugins/goalrail/")), mode, sha}
] | sort_by(.path)' "$tree_file" >"$payload_file" ||
  blocked "TREE_INVALID" "remote plugin tree could not be normalized"

for required in \
  .codex-plugin/plugin.json \
  skills/goalrail/SKILL.md \
  skills/goalrail/references/index.md \
  skills/goalrail/references/install.md \
  skills/goalrail/references/plugin-lifecycle.md \
  skills/goalrail/references/update-discovery.md \
  skills/goalrail/scripts/homebrew-update-state.sh \
  skills/goalrail/scripts/plugin-update-target.sh; do
  jq -e --arg path "$required" 'any(.[]; .path == $path)' "$payload_file" >/dev/null 2>&1 ||
    blocked "PAYLOAD_INCOMPLETE" "remote plugin tree is missing a required file"
done

manifest_blob=$(git hash-object "$manifest_file" 2>/dev/null) ||
  blocked "HASH_FAILED" "remote plugin manifest could not be hashed"
skill_blob=$(git hash-object "$skill_file" 2>/dev/null) ||
  blocked "HASH_FAILED" "remote Goalrail skill could not be hashed"
jq -e --arg sha "$manifest_blob" '
  any(.[]; .path == ".codex-plugin/plugin.json" and .sha == $sha)
' "$payload_file" >/dev/null 2>&1 ||
  blocked "MANIFEST_TREE_MISMATCH" "remote manifest bytes do not match the exact Git tree"
jq -e --arg sha "$skill_blob" '
  any(.[]; .path == "skills/goalrail/SKILL.md" and .sha == $sha)
' "$payload_file" >/dev/null 2>&1 ||
  blocked "SKILL_TREE_MISMATCH" "remote skill bytes do not match the exact Git tree"

tag_output=$(git ls-remote --exit-code "$repository" \
  "refs/tags/$candidate_ref" "refs/tags/$candidate_ref^{}" 2>/dev/null) ||
  blocked "TAG_UNRESOLVED" "advertised Goalrail tag could not be resolved"
tag_commit=$(printf '%s\n' "$tag_output" |
  awk -v ref="refs/tags/$candidate_ref^{}" '$2 == ref { print $1 }')
[ "$(printf '%s\n' "$tag_commit" | grep -Ec '^[0-9a-f]{40}$')" -eq 1 ] ||
  blocked "TAG_AMBIGUOUS" "advertised Goalrail tag has no unique peeled commit"
[ "$tag_commit" = "$remote_commit" ] ||
  blocked "TAG_COMMIT_MISMATCH" "advertised Goalrail tag does not resolve to the catalog commit"

cache_verified=null
if [ -n "$cache_root" ]; then
  [ -d "$cache_root" ] || blocked "CACHE_MISSING" "installed plugin cache root is absent"
  [ ! -L "$cache_root" ] || blocked "CACHE_SYMLINK_ROOT" "installed plugin cache root is a symlink"
  cache_symlink=$(find "$cache_root" -type l -print -quit 2>/dev/null) ||
    blocked "CACHE_SCAN_FAILED" "installed plugin cache symlinks could not be inspected"
  [ -z "$cache_symlink" ] || blocked "CACHE_SYMLINK" "installed plugin cache contains a symlink"
  cache_special=$(find "$cache_root" ! -type d ! -type f ! -type l -print -quit 2>/dev/null) ||
    blocked "CACHE_SCAN_FAILED" "installed plugin cache entries could not be inspected"
  [ -z "$cache_special" ] || blocked "CACHE_ENTRY_INVALID" "installed plugin cache contains a non-regular entry"
  jq -r '.[] | [.path, .mode, .sha] | @tsv' "$payload_file" >"$payload_tsv" ||
    blocked "TREE_INVALID" "remote plugin inventory could not be rendered"
  tab=$(printf '\t')
  while IFS="$tab" read -r relative mode expected_sha; do
    local_path="$cache_root/$relative"
    [ -f "$local_path" ] || blocked "CACHE_INCOMPLETE" "installed plugin cache is missing a payload file"
    observed_sha=$(git hash-object "$local_path" 2>/dev/null) ||
      blocked "HASH_FAILED" "installed plugin cache file could not be hashed"
    [ "$observed_sha" = "$expected_sha" ] ||
      blocked "CACHE_BLOB_MISMATCH" "installed plugin cache differs from the approved payload"
    if [ "$mode" = 100755 ]; then
      [ -x "$local_path" ] || blocked "CACHE_MODE_MISMATCH" "installed plugin helper is not executable"
    else
      [ ! -x "$local_path" ] || blocked "CACHE_MODE_MISMATCH" "installed plugin file has an unexpected executable mode"
    fi
  done <"$payload_tsv"
  expected_count=$(jq 'length' "$payload_file")
  observed_count=$(find "$cache_root" -type f | wc -l | tr -d ' ')
  [ "$observed_count" -eq "$expected_count" ] ||
    blocked "CACHE_INVENTORY_MISMATCH" "installed plugin cache has missing or extra files"
  cache_verified=true
fi

payload_inventory=$(git hash-object "$payload_file" 2>/dev/null) ||
  blocked "HASH_FAILED" "plugin payload inventory could not be hashed"
channel_lag=false
[ "$comparison" = lower ] && channel_lag=true
jq -cn \
  --arg manager "$manager" \
  --arg version "$candidate_version" \
  --arg ref "$candidate_ref" \
  --arg commit "$remote_commit" \
  --arg inventory "$payload_inventory" \
  --argjson files "$(jq 'length' "$payload_file")" \
  --argjson channel_lag "$channel_lag" \
  --argjson cache_verified "$cache_verified" '{
    schemaVersion: 1,
    verdict: "TARGET_VERIFIED",
    manager: $manager,
    remoteCandidate: {
      version: $version,
      ref: $ref,
      commit: $commit,
      payloadFiles: $files,
      payloadInventoryGitBlob: $inventory,
      channelLag: $channel_lag
    },
    cacheVerified: $cache_verified
  }'

#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
marketplace="$repo_root/.agents/plugins/marketplace.json"
plugin_root="$repo_root/plugins/goalrail"
manifest="$plugin_root/.codex-plugin/plugin.json"
skill="$plugin_root/skills/goalrail/SKILL.md"
index="$plugin_root/skills/goalrail/references/index.md"
install="$plugin_root/skills/goalrail/references/install.md"
plugin_lifecycle="$plugin_root/skills/goalrail/references/plugin-lifecycle.md"
remote_smoke="$repo_root/scripts/smoke-goalrail-plugin-remote.sh"

command -v jq >/dev/null 2>&1 || {
  echo "goalrail plugin test requires jq" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || {
  echo "goalrail plugin test requires cargo" >&2
  exit 1
}

plugin_version=$(jq -er '.version' "$manifest")
release_ref=v$plugin_version
workspace_versions=$(cargo metadata --locked --offline --no-deps --format-version 1 \
  --manifest-path "$repo_root/Cargo.toml" |
  jq -r '.packages[].version' | sort -u)

test "$workspace_versions" = "$plugin_version" || {
  echo "Goalrail workspace and plugin versions must match: workspace=$workspace_versions plugin=$plugin_version" >&2
  exit 1
}

jq -e --arg ref "$release_ref" '
  .name == "goalrail"
  and (.plugins | length) == 1
  and .plugins[0].name == "goalrail"
  and .plugins[0].source == {
    source: "git-subdir",
    url: "https://github.com/heurema/goalrail-rs.git",
    path: "./plugins/goalrail",
    ref: $ref
  }
  and .plugins[0].policy.installation == "AVAILABLE"
  and .plugins[0].policy.authentication == "ON_INSTALL"
  and .plugins[0].policy.products == ["CODEX"]
' "$marketplace" >/dev/null

jq -e --arg version "$plugin_version" '
  .name == "goalrail"
  and .version == $version
  and .author.name == "Heurema"
  and .skills == "./skills/"
  and .interface.displayName == "Goalrail"
  and .interface.category == "Developer Tools"
' "$manifest" >/dev/null

for file in "$skill" "$index" "$install" "$plugin_lifecycle"; do
  test -s "$file" || {
    echo "goalrail plugin file is missing or empty: $file" >&2
    exit 1
  }
done

test -x "$remote_smoke" || {
  echo "goalrail remote plugin smoke is missing or not executable: $remote_smoke" >&2
  exit 1
}

sed -n '1,8p' "$skill" | grep -F 'name: goalrail' >/dev/null
sed -n '1,8p' "$skill" | grep -F 'description:' >/dev/null

for command in \
  'gr inspect codex --json' \
  'gr inspect codex skills --actionable --json' \
  'gr inspect codex skills --json' \
  'gr inspect codex plugins --json'; do
  grep -F "$command" "$index" >/dev/null || {
    echo "goalrail agent index is missing command: $command" >&2
    exit 1
  }
done

grep -F 'https://goalrail.dev/install.md' "$install" >/dev/null
grep -F 'brew install heurema/tap/goalrail' "$install" >/dev/null
grep -F 'never install, update, or remove the Goalrail Codex plugin' "$install" >/dev/null
grep -F 'codex plugin marketplace add heurema/goalrail-rs --ref main --json' "$plugin_lifecycle" >/dev/null
grep -F 'codex plugin add goalrail@goalrail --json' "$plugin_lifecycle" >/dev/null
grep -F 'codex plugin remove goalrail@goalrail --json' "$plugin_lifecycle" >/dev/null
grep -F 'no `codex plugin update` command' "$plugin_lifecycle" >/dev/null
grep -F 'unchanged version and installed path: `NO_CHANGE`' "$plugin_lifecycle" >/dev/null
grep -F 'changed version with an enabled `goalrail@goalrail`: `UPDATED`' "$plugin_lifecycle" >/dev/null
grep -F 'atomically. The supported update path' "$plugin_lifecycle" >/dev/null
grep -F 'Homebrew installation never registers or installs this Codex plugin' "$plugin_lifecycle" >/dev/null
grep -F 'codex plugin marketplace add "$remote_source" --ref "$remote_ref" --json' "$remote_smoke" >/dev/null
grep -F 'payload_ref=v$expected_version' "$remote_smoke" >/dev/null
grep -F 'remote marketplace advertises an unexpected Goalrail payload' "$remote_smoke" >/dev/null
grep -F 'git ls-remote --exit-code "$payload_url"' "$remote_smoke" >/dev/null
grep -F '"skills/list"' "$remote_smoke" >/dev/null
grep -F '.name == "goalrail:goalrail"' "$remote_smoke" >/dev/null
grep -F 'skills/list did not expose the installed Goalrail skill' "$remote_smoke" >/dev/null
grep -F 'if the user says only "Goalrail", ask which target they mean' "$skill" >/dev/null
grep -F 'If `gr` is absent, read' "$skill" >/dev/null
grep -F 'gr inspect codex --help' "$skill" >/dev/null
grep -F 'Public Goalrail CLI `0.3.0` and later supports' "$index" >/dev/null

echo "GOALRAIL_PLUGIN_TEST_OK marketplace=goalrail plugin=goalrail skill=goalrail"

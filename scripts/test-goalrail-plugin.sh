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
update_discovery="$plugin_root/skills/goalrail/references/update-discovery.md"
homebrew_state="$plugin_root/skills/goalrail/scripts/homebrew-update-state.sh"
plugin_target="$plugin_root/skills/goalrail/scripts/plugin-update-target.sh"
public_install="$repo_root/crates/gr-site/public/install.md"
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
workspace_metadata=$(cargo metadata --locked --offline --no-deps --format-version 1 \
  --manifest-path "$repo_root/Cargo.toml")
workspace_versions=$(printf '%s\n' "$workspace_metadata" |
  jq -r '.packages[].version' | sort -u)

test "$workspace_versions" = "$plugin_version" || {
  echo "Goalrail workspace and plugin versions must match: workspace=$workspace_versions plugin=$plugin_version" >&2
  exit 1
}

dependency_contract='(
  [.packages[] | select(.name == "gr") | .dependencies[]
    | select(.name == "gr-inspect-codex") | .req]
  == [("^" + $version)]
)'
printf '%s\n' "$workspace_metadata" |
  jq -e --arg version "$plugin_version" "$dependency_contract" >/dev/null || {
  echo "Goalrail CLI dependency requirement must match the shared version" >&2
  exit 1
}
stale_requirement=$(printf '%s\n' "$workspace_metadata" | jq '
  (.packages[] | select(.name == "gr") | .dependencies[]
    | select(.name == "gr-inspect-codex") | .req) = "^0.3.8"
')
if printf '%s\n' "$stale_requirement" |
    jq -e --arg version "$plugin_version" "$dependency_contract" >/dev/null; then
  echo "Goalrail shared-version gate accepted a stale dependency requirement" >&2
  exit 1
fi

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

for file in "$skill" "$index" "$install" "$plugin_lifecycle" "$update_discovery" "$homebrew_state" "$plugin_target" "$public_install"; do
  test -s "$file" || {
    echo "goalrail plugin file is missing or empty: $file" >&2
    exit 1
  }
done

test -x "$homebrew_state" || {
  echo "Goalrail Homebrew update-state helper is not executable: $homebrew_state" >&2
  exit 1
}

test -x "$plugin_target" || {
  echo "Goalrail plugin update-target helper is not executable: $plugin_target" >&2
  exit 1
}

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
grep -F 'lifecycle mutation where the user says only "Goalrail"' "$skill" >/dev/null
grep -F 'If `gr` is absent, read' "$skill" >/dev/null
grep -F 'gr inspect codex --help' "$skill" >/dev/null
grep -F 'Public Goalrail CLI `0.3.0` and later supports' "$index" >/dev/null
grep -F 'check for Goalrail updates' "$skill" >/dev/null
grep -F 'read `references/update-discovery.md`' "$skill" >/dev/null
grep -F 'observation flow in that reference' "$skill" >/dev/null
grep -F 'Codex host' "$skill" >/dev/null
grep -F 'brew tap-info --json heurema/tap' "$update_discovery" >/dev/null
grep -F 'git ls-remote --exit-code https://github.com/heurema/homebrew-tap.git refs/heads/main' "$update_discovery" >/dev/null
grep -F 'scripts/homebrew-update-state.sh' "$update_discovery" >/dev/null
grep -F 'only Goalrail-owned interpreter of `brew info` and `brew outdated`' "$update_discovery" >/dev/null
grep -F '`brew info` reads the local tap' "$update_discovery" >/dev/null
grep -F 'status --porcelain --untracked-files=no' "$update_discovery" >/dev/null
grep -F 'symbolic-ref --short HEAD' "$update_discovery" >/dev/null
grep -F 'their absence is not a' "$update_discovery" >/dev/null
grep -F '<codex-home>/config.toml' "$update_discovery" >/dev/null
grep -F 'plugins/cache/goalrail/goalrail/<version>/.codex-plugin/plugin.json' "$update_discovery" >/dev/null
grep -F '.codex-marketplace-install.json' "$update_discovery" >/dev/null
grep -F 'ref_name` `main`' "$update_discovery" >/dev/null
grep -F 'Peel an annotated tag' "$update_discovery" >/dev/null
grep -F 'report `STALE_METADATA`, leave' "$update_discovery" >/dev/null
grep -F 'codex plugin list --marketplace goalrail --json' "$update_discovery" >/dev/null
grep -F 'neither changed its commit nor `config.toml`' "$update_discovery" >/dev/null
grep -F 'unexpected change is `HOST_RECONCILED`' "$update_discovery" >/dev/null
grep -F '`startupReconciliation`' "$update_discovery" >/dev/null
grep -F 'Their absence alone is not a failure' "$update_discovery" >/dev/null
grep -F 'Never promise that opening or restarting a Codex task' "$update_discovery" >/dev/null
grep -F '`HOST_RECONCILED`' "$update_discovery" >/dev/null
grep -F 'https://api.github.com/repos/heurema/goalrail-rs/releases/latest' "$update_discovery" >/dev/null
grep -F 'common `latestRelease`' "$update_discovery" >/dev/null
grep -F '`CHANNEL_LAG`' "$update_discovery" >/dev/null
grep -F '`nextAction: WAIT_FOR_CHANNEL`' "$update_discovery" >/dev/null
grep -F '`channelLag: true`' "$update_discovery" >/dev/null
grep -F '`nextAction: REQUEST_HOMEBREW_UPGRADE_APPROVAL`' "$update_discovery" >/dev/null
grep -F '`nextAction: REQUEST_PLUGIN_ADD_APPROVAL`' "$update_discovery" >/dev/null
grep -F '`NO_CHANGE` requires a fresh public release baseline' "$update_discovery" >/dev/null
grep -F 'Never let an exposed catalog version satisfy overall `NO_CHANGE`' "$update_discovery" >/dev/null
grep -F 'path spelled exactly `plugins/goalrail` or `./plugins/goalrail`' "$update_discovery" >/dev/null
grep -F 'including an absolute path,' "$update_discovery" >/dev/null
grep -F '`nextAction: MANUAL_REINSTALL_REQUIRED`' "$update_discovery" >/dev/null
grep -F 'DIRECT_RELEASE_UNMANAGED' "$update_discovery" >/dev/null
grep -F 'Do not download an archive, overwrite the' "$update_discovery" >/dev/null
grep -F 'codex plugin marketplace upgrade goalrail --json' "$update_discovery" >/dev/null
grep -F 'potentially combined catalog-and-plugin reconciliation' "$update_discovery" >/dev/null
grep -F 'scripts/plugin-update-target.sh' "$update_discovery" >/dev/null
grep -F 'Report `remoteCandidate`, not' "$update_discovery" >/dev/null
grep -F 'skill tree, Git blob inventory' "$update_discovery" >/dev/null
grep -F '`cacheVerified: true`' "$update_discovery" >/dev/null
grep -F 'not run `plugin add`' "$update_discovery" >/dev/null
grep -F 'calling either list command' "$update_discovery" >/dev/null
grep -F 'cannot eliminate' "$update_discovery" >/dev/null
grep -F 'Post-command exact snapshot and cache verification remains mandatory' "$update_discovery" >/dev/null
grep -F 'open a new task or restart the' "$update_discovery" >/dev/null
grep -F 'Goalrail skill and CLI perform no background update check' "$update_discovery" >/dev/null
grep -F 'read `update-discovery.md`' "$install" >/dev/null
grep -F 'follow `update-discovery.md`' "$plugin_lifecycle" >/dev/null
grep -F 'pre/post evidence' "$plugin_lifecycle" >/dev/null
grep -F 'For update intent,' "$plugin_lifecycle" >/dev/null
grep -F 'codex plugin list --marketplace goalrail --json' "$plugin_lifecycle" >/dev/null
grep -F 'potentially combined state-changing action' "$plugin_lifecycle" >/dev/null
grep -F 'do not run `plugin add`' "$plugin_lifecycle" >/dev/null
grep -F 'scripts/plugin-update-target.sh' "$plugin_lifecycle" >/dev/null
grep -F 'cannot atomically bind its moving ref' "$plugin_lifecycle" >/dev/null
grep -F 'git -C <tap-root> symbolic-ref --short HEAD' "$public_install" >/dev/null
grep -F 'git -C <tap-root> rev-parse HEAD' "$public_install" >/dev/null
grep -F '`channelLag: true`' "$public_install" >/dev/null
grep -F 'fresh formula version equals both the installed version and the latest' "$public_install" >/dev/null
grep -F 'update both the marketplace snapshot and' "$public_install" >/dev/null
grep -F 'update is complete' "$public_install" >/dev/null
grep -F '`plugin add` must not be run only after that proof' "$public_install" >/dev/null
grep -F 'scripts/plugin-update-target.sh' "$public_install" >/dev/null
grep -F 'does not atomically bind marketplace upgrade' "$public_install" >/dev/null

for file in "$update_discovery" "$plugin_lifecycle" "$public_install"; do
  if grep -F 'refresh and plugin application remain two separately approved' "$file" >/dev/null \
      || grep -F 'refreshes only the Goalrail marketplace' "$file" >/dev/null \
      || grep -F 'Applying an available plugin update is another separately approved action' "$file" >/dev/null; then
    echo "Goalrail lifecycle docs still claim marketplace upgrade is metadata-only: $file" >&2
    exit 1
  fi
done

if grep -F 'Discovery is read-only.' "$update_discovery" >/dev/null; then
  echo "Goalrail plugin update discovery must not promise an end-to-end read-only Codex lifecycle" >&2
  exit 1
fi

echo "GOALRAIL_PLUGIN_TEST_OK marketplace=goalrail plugin=goalrail skill=goalrail"

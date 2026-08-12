#!/bin/sh

set -u

formula=heurema/tap/goalrail

blocked() {
  code=$1
  message=$2
  jq -cn --arg code "$code" --arg message "$message" '{
    schemaVersion: 1,
    verdict: "BLOCKED",
    manager: "HOMEBREW",
    finding: { code: $code, message: $message }
  }'
  exit 4
}

command -v jq >/dev/null 2>&1 || {
  printf '%s\n' '{"schemaVersion":1,"verdict":"BLOCKED","manager":"HOMEBREW","finding":{"code":"JQ_MISSING","message":"jq is unavailable"}}'
  exit 4
}
command -v brew >/dev/null 2>&1 || blocked "HOMEBREW_MISSING" "brew is unavailable"

probe_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-brew-state.XXXXXX") ||
  blocked "TEMP_UNAVAILABLE" "temporary probe storage could not be created"
info_file="$probe_root/info.json"
outdated_file="$probe_root/outdated.json"
cleanup() {
  rm -f "$info_file" "$outdated_file"
  rmdir "$probe_root" 2>/dev/null || true
}
trap cleanup EXIT HUP INT TERM

if ! HOMEBREW_NO_AUTO_UPDATE=1 brew info --json=v2 "$formula" >"$info_file" 2>/dev/null; then
  blocked "INFO_FAILED" "brew info failed"
fi
if ! jq -e '
  (.formulae | type == "array")
  and (.casks | type == "array")
  and (.casks | length) == 0
  and ([.formulae[] | select(.full_name == "heurema/tap/goalrail")] | length) == 1
  and (.formulae | length) == 1
  and (.formulae[0].versions.stable | type == "string")
  and (.formulae[0].installed | type == "array")
' "$info_file" >/dev/null 2>&1; then
  blocked "INFO_INVALID" "brew info returned malformed or ambiguous Goalrail JSON"
fi

available_version=$(jq -er '.formulae[0].versions.stable' "$info_file") ||
  blocked "INFO_INVALID" "brew info omitted the stable Goalrail version"
if ! printf '%s\n' "$available_version" |
  grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'; then
  blocked "VERSION_INVALID" "brew info returned a non-stable Goalrail version"
fi

installed_json=$(jq -c '[.formulae[0].installed[]?.version] | unique | sort' "$info_file") ||
  blocked "INFO_INVALID" "brew info returned invalid installed versions"
if ! printf '%s\n' "$installed_json" | jq -e '
  type == "array" and all(.[];
    type == "string"
    and test("^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$"))
' >/dev/null 2>&1; then
  blocked "VERSION_INVALID" "brew info returned a non-stable installed version"
fi

if [ "$(printf '%s\n' "$installed_json" | jq 'length')" -eq 0 ]; then
  jq -cn --arg available "$available_version" '{
    schemaVersion: 1,
    verdict: "NOT_INSTALLED",
    manager: "HOMEBREW",
    installedVersions: [],
    availableVersion: $available,
    outdated: false,
    outdatedExit: null
  }'
  exit 0
fi

outdated_status=0
HOMEBREW_NO_AUTO_UPDATE=1 brew outdated --json=v2 "$formula" >"$outdated_file" 2>/dev/null ||
  outdated_status=$?
case "$outdated_status" in
  0|1) ;;
  *) blocked "OUTDATED_FAILED" "brew outdated returned an unsupported exit status" ;;
esac

if ! jq -e '
  (.formulae | type == "array")
  and (.casks | type == "array")
  and (.casks | length) == 0
  and all(.formulae[]; .name == "goalrail" or .name == "heurema/tap/goalrail")
  and (.formulae | length) <= 1
' "$outdated_file" >/dev/null 2>&1; then
  blocked "OUTDATED_INVALID" "brew outdated returned malformed or ambiguous Goalrail JSON"
fi

outdated_count=$(jq '.formulae | length' "$outdated_file")
case "$outdated_status:$outdated_count" in
  0:0)
    if ! printf '%s\n' "$installed_json" | jq -e --arg available "$available_version" 'index($available) != null' >/dev/null; then
      blocked "STATE_CONTRADICTION" "brew reports no update but the stable version is not installed"
    fi
    jq -cn --argjson installed "$installed_json" --arg available "$available_version" '{
      schemaVersion: 1,
      verdict: "NO_CHANGE",
      manager: "HOMEBREW",
      installedVersions: $installed,
      availableVersion: $available,
      outdated: false,
      outdatedExit: 0
    }'
    ;;
  1:1)
    current_version=$(jq -er '.formulae[0].current_version' "$outdated_file") ||
      blocked "OUTDATED_INVALID" "brew outdated omitted the current version"
    outdated_installed=$(jq -c '.formulae[0].installed_versions | unique | sort' "$outdated_file") ||
      blocked "OUTDATED_INVALID" "brew outdated omitted installed versions"
    [ "$current_version" = "$available_version" ] ||
      blocked "STATE_CONTRADICTION" "brew info and brew outdated disagree on the available version"
    [ "$outdated_installed" = "$installed_json" ] ||
      blocked "STATE_CONTRADICTION" "brew info and brew outdated disagree on installed versions"
    jq -cn --argjson installed "$installed_json" --arg available "$available_version" '{
      schemaVersion: 1,
      verdict: "UPDATE_AVAILABLE",
      manager: "HOMEBREW",
      installedVersions: $installed,
      availableVersion: $available,
      outdated: true,
      outdatedExit: 1
    }'
    ;;
  *) blocked "STATE_CONTRADICTION" "brew outdated exit status contradicts its JSON content" ;;
esac

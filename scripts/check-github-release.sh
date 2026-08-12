#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <draft-partial|draft|published> <run-id> <run-attempt> [output-root] [remote]" >&2
  exit 64
}

fail() {
  echo "check-github-release: $1" >&2
  exit 1
}

checksum() {
  file=$1

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    fail "no SHA-256 command is available"
  fi
}

if [ "$#" -lt 4 ] || [ "$#" -gt 6 ]; then
  usage
fi

version=$1
release_state=$2
expected_run_id=$3
expected_run_attempt=$4
output_root=${5:-dist}
remote=${6:-origin}

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi
if ! printf '%s\n' "$expected_run_id" | grep -Eq '^[1-9][0-9]*$'; then
  fail "run ID must be a positive integer"
fi
if ! printf '%s\n' "$expected_run_attempt" | grep -Eq '^[1-9][0-9]*$'; then
  fail "run attempt must be a positive integer"
fi
case "$release_state" in
  draft-partial)
    expected_draft=true
    require_complete=false
    verdict=DRAFT_PARTIAL_VERIFIED
    ;;
  draft) expected_draft=true; require_complete=true; verdict=DRAFT_VERIFIED ;;
  published) expected_draft=false; require_complete=true; verdict=PUBLISHED_VERIFIED ;;
  *) usage ;;
esac

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v gh >/dev/null 2>&1 || fail "gh is unavailable"
command -v jq >/dev/null 2>&1 || fail "jq is unavailable"

tag="v$version"
release_dir="$output_root/$tag"
release_manifest="$release_dir/release.json"
[ -f "$release_manifest" ] || fail "release manifest is missing: $release_manifest"
source_commit=$(jq -er '.sourceCommit' "$release_manifest")
run_id=$(jq -er '.run.id' "$release_manifest")
run_attempt=$(jq -er '.run.attempt' "$release_manifest")
"$script_dir/check-release-bundle.sh" \
  "$version" "$source_commit" "$expected_run_id" "$expected_run_attempt" \
  "$output_root" >/dev/null
[ "$run_id" = "$expected_run_id" ] || fail "release manifest run ID does not match"
[ "$run_attempt" = "$expected_run_attempt" ] ||
  fail "release manifest run attempt does not match"
"$script_dir/check-remote-release-tag.sh" \
  "$tag" "$source_commit" "$remote" >/dev/null

release_json=$(gh release view "$tag" \
  --json tagName,isDraft,isPrerelease,assets) ||
  fail "GitHub Release could not be read: $tag"

printf '%s\n' "$release_json" | jq -e \
  --arg tag "$tag" \
  --argjson expected_draft "$expected_draft" '
    .tagName == $tag
    and .isDraft == $expected_draft
    and .isPrerelease == false
  ' >/dev/null || fail "GitHub Release state does not match"

assets="
goalrail-$tag-aarch64-apple-darwin.tar.gz
goalrail-$tag-aarch64-apple-darwin.tar.gz.sha256
goalrail-$tag-aarch64-apple-darwin.json
goalrail-$tag-x86_64-unknown-linux-gnu.tar.gz
goalrail-$tag-x86_64-unknown-linux-gnu.tar.gz.sha256
goalrail-$tag-x86_64-unknown-linux-gnu.json
goalrail-$tag-x86_64-pc-windows-msvc.tar.gz
goalrail-$tag-x86_64-pc-windows-msvc.tar.gz.sha256
goalrail-$tag-x86_64-pc-windows-msvc.json
goalrail.rb
release.json
"

comparison_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-public-release.XXXXXX")
cleanup() {
  rm -rf "$comparison_dir"
}
trap cleanup EXIT HUP INT TERM

expected_names_file="$comparison_dir/expected"
actual_names_file="$comparison_dir/actual"
unique_actual_names_file="$comparison_dir/actual-unique"
missing_names_file="$comparison_dir/missing"
printf '%s\n' "$assets" | sed '/^$/d' | sort >"$expected_names_file"
printf '%s\n' "$release_json" |
  jq -r '.assets[].name' | sort >"$actual_names_file"
sort -u "$actual_names_file" >"$unique_actual_names_file"
cmp -s "$actual_names_file" "$unique_actual_names_file" ||
  fail "GitHub Release contains duplicate asset names"
[ -z "$(comm -13 "$expected_names_file" "$unique_actual_names_file")" ] ||
  fail "GitHub Release contains an unexpected asset"
comm -23 "$expected_names_file" "$unique_actual_names_file" >"$missing_names_file"
if [ "$require_complete" = true ] && [ -s "$missing_names_file" ]; then
  fail "GitHub Release asset set is incomplete"
fi

printf '%s\n' "$assets" | while IFS= read -r asset; do
  [ -n "$asset" ] || continue
  local_file="$release_dir/$asset"
  [ -f "$local_file" ] || fail "local release asset is missing: $local_file"
  if ! grep -Fx "$asset" "$actual_names_file" >/dev/null; then
    continue
  fi
  local_digest="sha256:$(checksum "$local_file")"
  local_size=$(wc -c <"$local_file" | tr -d ' ')
  printf '%s\n' "$release_json" | jq -e \
    --arg name "$asset" \
    --arg digest "$local_digest" \
    --argjson size "$local_size" '
      [.assets[] | select(.name == $name)] as $matches
      | ($matches | length) == 1
        and $matches[0].state == "uploaded"
        and $matches[0].digest == $digest
        and $matches[0].size == $size
    ' >/dev/null || fail "GitHub Release asset digest or size does not match: $asset"
done

observed_assets=$(wc -l <"$actual_names_file" | tr -d ' ')
missing_json=$(jq -Rsc 'split("\n") | map(select(length > 0))' \
  <"$missing_names_file")
printf '{"schemaVersion":2,"verdict":"%s","version":"%s","tag":"%s","sourceCommit":"%s","runId":"%s","runAttempt":"%s","expectedAssets":11,"observedAssets":%s,"missing":%s}\n' \
  "$verdict" "$version" "$tag" "$source_commit" "$run_id" "$run_attempt" \
  "$observed_assets" "$missing_json"

#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version>" >&2
  exit 64
}

[ "$#" -eq 1 ] || usage
version=$1

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  usage
fi

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

checksum() {
  file=$1

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 "$file" | awk '{print $NF}'
  else
    return 1
  fi
}

findings_file=$(mktemp "${TMPDIR:-/tmp}/goalrail-preflight.XXXXXX")
cleanup() {
  rm -f "$findings_file"
}
trap cleanup EXIT HUP INT TERM

add_finding() {
  printf '%s|%s\n' "$1" "$2" >>"$findings_file"
}

if ! command -v cargo >/dev/null 2>&1; then
  add_finding "CARGO_MISSING" "cargo is required to resolve the release version"
elif package_id=$(cargo pkgid -p gr 2>/dev/null); then
  crate_version=${package_id##*@}
  [ "$crate_version" = "$version" ] ||
    add_finding "VERSION_MISMATCH" "requested version does not match the gr crate"
else
  add_finding "PACKAGE_UNRESOLVED" "the gr package version could not be resolved"
fi

if [ ! -f LICENSE ] && [ ! -f LICENSE.md ]; then
  add_finding "LICENSE_MISSING" "distribution terms must be selected before publication"
fi

for script in \
  scripts/check-release-candidate.sh \
  scripts/check-github-release.sh \
  scripts/check-github-run.sh \
  scripts/check-remote-release-tag-absent.sh \
  scripts/prepare-release.sh \
  scripts/package-release.sh \
  scripts/check-release.sh \
  scripts/assemble-release.sh \
  scripts/check-release-bundle.sh \
  scripts/check-remote-release-tag.sh \
  scripts/promote-homebrew.sh \
  scripts/smoke-release-binary.sh; do
  [ -x "$script" ] ||
    add_finding "RELEASE_TOOL_MISSING" "required executable is missing: $script"
done

[ -f .github/workflows/release.yml ] ||
  add_finding "RELEASE_WORKFLOW_MISSING" "manual multi-platform build workflow is missing"

if ! grep -Eq '^[[:space:]]+license "[^"]+"' release/homebrew/goalrail.rb.in; then
  add_finding "FORMULA_LICENSE_MISSING" "the Homebrew formula must declare the selected license"
fi

if ! git diff --quiet || ! git diff --cached --quiet ||
  [ -n "$(git ls-files --others --exclude-standard)" ]; then
  add_finding "WORKTREE_DIRTY" "the release source must be a clean checkout"
fi

tag="v$version"
if git show-ref --verify --quiet "refs/tags/$tag"; then
  add_finding "LOCAL_TAG_EXISTS" "the candidate version already has a local tag"
fi

if remote_refs=$(git ls-remote origin \
  "refs/tags/$tag" "refs/tags/$tag^{}" 2>/dev/null); then
  if [ -n "$remote_refs" ]; then
    add_finding "REMOTE_TAG_EXISTS" "the candidate version already has a remote tag"
  fi
else
  add_finding "REMOTE_TAG_QUERY_FAILED" "the origin release tags could not be queried"
fi

if [ -s "$findings_file" ]; then
  printf '{"schemaVersion":3,"verdict":"BLOCKED","version":"%s","findings":[' "$version"
  first=1
  while IFS='|' read -r code message; do
    if [ "$first" -eq 0 ]; then
      printf ','
    fi
    first=0
    printf '{"code":"%s","message":"%s"}' "$code" "$message"
  done <"$findings_file"
  printf ']}\n'
  exit 4
fi

source_commit=$(git rev-parse HEAD)
lock_sha256=$(checksum Cargo.lock) || {
  printf '{"schemaVersion":3,"verdict":"BLOCKED","version":"%s","findings":[{"code":"CHECKSUM_TOOL_MISSING","message":"no SHA-256 command is available"}]}\n' "$version"
  exit 4
}
printf '{"schemaVersion":3,"verdict":"READY_FOR_CANDIDATE","version":"%s","plannedTag":"%s","sourceCommit":"%s","cargoLockSha256":"%s","findings":[]}\n' \
  "$version" "$tag" "$source_commit" "$lock_sha256"

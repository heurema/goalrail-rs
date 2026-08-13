#!/bin/sh

set -eu

fail() {
  echo "assessment-boundary: $1" >&2
  exit 1
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
assessment_manifest=${GOALRAIL_ASSESSMENT_MANIFEST:-"$repo_root/crates/gr-skill-assessment/Cargo.toml"}
metadata_file=${GOALRAIL_CARGO_METADATA_JSON:-}

if [ "${GOALRAIL_ASSESSMENT_GATE_TESTING:-0}" != 1 ] &&
  { [ -n "${GOALRAIL_ASSESSMENT_MANIFEST:-}" ] ||
    [ -n "${GOALRAIL_CARGO_METADATA_JSON:-}" ]; }; then
  fail "test overrides require GOALRAIL_ASSESSMENT_GATE_TESTING=1"
fi

work_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-assessment-boundary.XXXXXX")

cleanup() {
  rm -rf "$work_root"
}
trap cleanup EXIT HUP INT TERM

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"

if [ -z "$metadata_file" ]; then
  metadata_file="$work_root/metadata.json"
  cargo metadata --format-version 1 --no-deps >"$metadata_file"
fi

jq -e '
  def package($name): first(.packages[] | select(.name == $name));
  def owned_edges:
    [
      .packages[] as $package
      | $package.dependencies[]
      | select(.source == null)
      | ($package.name + "->" + .name)
    ] | sort;
  def assessment_dependencies:
    package("gr-skill-assessment").dependencies;

  package("gr-skill-assessment").publish == []
  and package("gr-skill-assessment").features == {}
  and owned_edges == [
    "gr->gr-inspect-claude",
    "gr->gr-inspect-codex",
    "gr->gr-inspect-core",
    "gr-inspect-claude->gr-inspect-core",
    "gr-inspect-codex->gr-inspect-core",
    "gr-inspect-codex->gr-skill-assessment"
  ]
  and (assessment_dependencies | length) == 1
  and assessment_dependencies[0].name == "serde"
  and assessment_dependencies[0].uses_default_features == false
  and (assessment_dependencies[0].features | sort) == ["alloc", "derive"]
  and ([package("gr-skill-assessment").targets[].kind[]] | sort) == ["lib"]
' "$metadata_file" >/dev/null ||
  fail "Cargo metadata violates the accepted assessment dependency boundary"

host_target=$(rustc -vV | awk '/^host:/ { print $2 }')
source_lib="$(rustc --print sysroot)/lib/rustlib/$host_target/lib"
target_lib="$work_root/sysroot/lib/rustlib/$host_target/lib"
mkdir -p "$target_lib"

for library in "$source_lib"/*; do
  library_name=${library##*/}
  case "$library_name" in
    libstd-*|libstd_detect-*|libtest-*) continue ;;
  esac
  ln -s "$library" "$target_lib/$library_name"
done

cargo rustc --quiet --locked --offline --manifest-path "$assessment_manifest" --lib -- \
  --sysroot "$work_root/sysroot" --emit=metadata ||
  fail "gr-skill-assessment does not compile without std in the sysroot"

echo "ASSESSMENT_BOUNDARY_OK owned_edges=gr->gr-inspect-claude,gr->gr-inspect-codex,gr->gr-inspect-core,gr-inspect-claude->gr-inspect-core,gr-inspect-codex->gr-inspect-core,gr-inspect-codex->gr-skill-assessment assessment_deps=serde std=no"

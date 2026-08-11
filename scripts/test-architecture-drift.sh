#!/bin/sh

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
checker="$script_dir/architecture-drift.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-architecture-drift-test.XXXXXX")

cleanup() {
  rm -rf "$test_root"
}
trap cleanup EXIT HUP INT TERM

fixture="$test_root/fixture"
mkdir -p "$fixture/crates/app/src" "$fixture/crates/core/src" "$fixture/architecture/drift"
fixture=$(CDPATH= cd -- "$fixture" && pwd)
printf '%s\n' '[workspace]' >"$fixture/Cargo.toml"
printf '%s\n' '[package]' >"$fixture/crates/app/Cargo.toml"
printf '%s\n' '[package]' >"$fixture/crates/core/Cargo.toml"
printf '%s\n' 'pub fn run() {}' >"$fixture/crates/app/src/lib.rs"
printf '%s\n' 'pub struct Value;' >"$fixture/crates/core/src/lib.rs"
printf '%s\n' 'pub fn fixture::stable()' >"$fixture/public-api.txt"

metadata="$fixture/metadata.json"
jq -n --arg root "$fixture" '{
  packages: [
    {
      id: "app 0.1.0 (path+file:///fixture/app)",
      name: "app",
      manifest_path: ($root + "/crates/app/Cargo.toml"),
      dependencies: [
        {
          name: "core",
          source: null,
          path: ($root + "/crates/core"),
          kind: null,
          target: null,
          optional: false
        }
      ]
    },
    {
      id: "core 0.1.0 (path+file:///fixture/core)",
      name: "core",
      manifest_path: ($root + "/crates/core/Cargo.toml"),
      dependencies: []
    }
  ],
  workspace_members: [
    "app 0.1.0 (path+file:///fixture/app)",
    "core 0.1.0 (path+file:///fixture/core)"
  ]
}' >"$metadata"

baseline="$fixture/architecture/drift/baseline.json"

run_trial() {
  trial_mode=$1
  trial_metadata=${2:-$metadata}
  trial_baseline=${3:-$baseline}
  GOALRAIL_ARCHITECTURE_DRIFT_TESTING=1 \
    GOALRAIL_ARCHITECTURE_DRIFT_ROOT="$fixture" \
    GOALRAIL_ARCHITECTURE_DRIFT_METADATA_JSON="$trial_metadata" \
    GOALRAIL_ARCHITECTURE_DRIFT_PUBLIC_API_FILE="$fixture/public-api.txt" \
    GOALRAIL_ARCHITECTURE_DRIFT_BASELINE="$trial_baseline" \
    "$checker" "$trial_mode"
}

run_trial capture >"$baseline"

unchanged="$test_root/unchanged.json"
run_trial check >"$unchanged"
jq -e '
  .advisory == true
  and .verdict == "NO_CHANGE"
  and .change_count == 0
  and .current_summary.workspace_members == 2
  and .current_summary.owned_edges == 1
  and .current_summary.public_api_items == 1
  and .current_summary.rust_source_files == 2
' "$unchanged" >/dev/null

reassigned_baseline="$test_root/reassigned-baseline.json"
jq '
  (.source_files[] | select(.path == "crates/app/src/lib.rs").package) = "core"
' "$baseline" >"$reassigned_baseline"
run_trial check "$metadata" "$reassigned_baseline" >"$test_root/reassigned.json"
jq -e '
  .verdict == "REVIEW"
  and .changes.source_files.changed == [{
    path: "crates/app/src/lib.rs",
    before: {package: "core", lines: 1},
    after: {package: "app", lines: 1},
    line_delta: 0
  }]
' "$test_root/reassigned.json" >/dev/null

printf '%s\n' 'pub fn renamed() {}' >"$fixture/crates/app/src/lib.rs"
same_size_drift="$test_root/same-size-drift.json"
run_trial check >"$same_size_drift"
jq -e '
  .verdict == "REVIEW"
  and .changes.source_files.changed == [{
    path: "crates/app/src/lib.rs",
    before: {package: "app", lines: 1},
    after: {package: "app", lines: 1},
    line_delta: 0
  }]
' "$same_size_drift" >/dev/null

printf '%s\n' 'pub fn second() {}' >>"$fixture/crates/app/src/lib.rs"
source_drift="$test_root/source-drift.json"
run_trial check >"$source_drift"
jq -e '
  .verdict == "REVIEW"
  and .changes.source_files.changed == [{
    path: "crates/app/src/lib.rs",
    before: {package: "app", lines: 1},
    after: {package: "app", lines: 2},
    line_delta: 1
  }]
' "$source_drift" >/dev/null

edge_metadata="$test_root/edge-metadata.json"
jq --arg root "$fixture" '
  (.packages[] | select(.name == "core").dependencies) += [{
    name: "app",
    source: null,
    path: ($root + "/crates/app"),
    kind: "dev",
    target: null,
    optional: false
  }]
' "$metadata" >"$edge_metadata"
run_trial check "$edge_metadata" >"$test_root/edge-drift.json"
jq -e '
  .changes.owned_edges.added == [{
    from: "core",
    to: "app",
    kind: "dev",
    target: null,
    optional: false
  }]
' "$test_root/edge-drift.json" >/dev/null

registry_collision_metadata="$test_root/registry-collision-metadata.json"
jq '
  (.packages[] | select(.name == "core").dependencies) += [{
    name: "app",
    source: "registry+https://github.com/rust-lang/crates.io-index",
    path: null,
    kind: null,
    target: null,
    optional: false
  }]
' "$metadata" >"$registry_collision_metadata"
run_trial check "$registry_collision_metadata" >"$test_root/registry-collision.json"
jq -e '.changes.owned_edges == {added: [], removed: []}' \
  "$test_root/registry-collision.json" >/dev/null

printf '%s\n' 'pub struct fixture::Added' >>"$fixture/public-api.txt"
run_trial check >"$test_root/public-api-drift.json"
jq -e '
  .changes.public_api_items.added == ["pub struct fixture::Added"]
' "$test_root/public-api-drift.json" >/dev/null

printf '%s\n' 'pub fn worker() {}' >"$fixture/crates/app/src/worker.rs"
run_trial check >"$test_root/file-drift.json"
jq -e '
  .changes.source_files.added == [{
    path: "crates/app/src/worker.rs",
    package: "app",
    lines: 1
  }]
' "$test_root/file-drift.json" >/dev/null

nested_root="$fixture/crates/app/nested"
mkdir -p "$nested_root/src"
printf '%s\n' '[package]' >"$nested_root/Cargo.toml"
printf '%s\n' 'pub struct Nested;' >"$nested_root/src/lib.rs"
nested_metadata="$test_root/nested-metadata.json"
jq --arg root "$fixture" '
  .packages += [{
    id: "nested 0.1.0 (path+file:///fixture/app/nested)",
    name: "nested",
    manifest_path: ($root + "/crates/app/nested/Cargo.toml"),
    dependencies: []
  }]
  | .workspace_members += [
      "nested 0.1.0 (path+file:///fixture/app/nested)"
    ]
' "$metadata" >"$nested_metadata"
run_trial check "$nested_metadata" >"$test_root/nested-package.json"
jq -e '
  .verdict == "REVIEW"
  and ([
    .changes.source_files.added[]
    | select(.path == "crates/app/nested/src/lib.rs")
  ] == [{
    path: "crates/app/nested/src/lib.rs",
    package: "nested",
    lines: 1
  }])
' "$test_root/nested-package.json" >/dev/null
rm -rf "$nested_root"

invalid_baseline="$test_root/invalid-baseline.json"
printf '%s\n' '{}' >"$invalid_baseline"
if run_trial check "$metadata" "$invalid_baseline" >/dev/null 2>&1; then
  echo "architecture drift accepted an invalid baseline" >&2
  exit 1
fi

duplicate_baseline="$test_root/duplicate-baseline.json"
jq '.workspace_members += [.workspace_members[0]]' "$baseline" >"$duplicate_baseline"
if run_trial check "$metadata" "$duplicate_baseline" >/dev/null 2>&1; then
  echo "architecture drift accepted a noncanonical duplicate baseline" >&2
  exit 1
fi

missing_package_baseline="$test_root/missing-package-baseline.json"
jq 'del(.source_files[0].package)' "$baseline" >"$missing_package_baseline"
if run_trial check "$metadata" "$missing_package_baseline" >/dev/null 2>&1; then
  echo "architecture drift accepted a baseline without source package identity" >&2
  exit 1
fi

missing_package_metadata="$test_root/missing-package-metadata.json"
jq --arg root "$fixture" '
  (.packages[] | select(.name == "app").manifest_path) =
    ($root + "/missing/Cargo.toml")
' "$metadata" >"$missing_package_metadata"
if run_trial check "$missing_package_metadata" >/dev/null 2>&1; then
  echo "architecture drift accepted a missing workspace package directory" >&2
  exit 1
fi

if GOALRAIL_ARCHITECTURE_DRIFT_ROOT="$fixture" "$checker" check >/dev/null 2>&1; then
  echo "architecture drift accepted an unguarded input override" >&2
  exit 1
fi

echo "ARCHITECTURE_DRIFT_TEST_OK sabotage=package-reassignment,same-size-source,source-lines,owned-edge,registry-name-collision,public-api,new-file,nested-package,invalid-baseline,noncanonical-baseline,missing-package,unguarded-override"

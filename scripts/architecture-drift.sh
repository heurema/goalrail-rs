#!/bin/sh

set -eu

fail() {
  echo "architecture-drift: $1" >&2
  exit 2
}

mode=${1:-check}
case "$mode" in
  capture|check) ;;
  *) fail "expected 'capture' or 'check', got '$mode'" ;;
esac

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd)
root=${GOALRAIL_ARCHITECTURE_DRIFT_ROOT:-"$repo_root"}
metadata_override=${GOALRAIL_ARCHITECTURE_DRIFT_METADATA_JSON:-}
public_api_override=${GOALRAIL_ARCHITECTURE_DRIFT_PUBLIC_API_FILE:-}
baseline=${GOALRAIL_ARCHITECTURE_DRIFT_BASELINE:-"$repo_root/architecture/drift/baseline.json"}

if [ "${GOALRAIL_ARCHITECTURE_DRIFT_TESTING:-0}" != 1 ] &&
  { [ -n "${GOALRAIL_ARCHITECTURE_DRIFT_ROOT:-}" ] ||
    [ -n "$metadata_override" ] ||
    [ -n "$public_api_override" ] ||
    [ -n "${GOALRAIL_ARCHITECTURE_DRIFT_BASELINE:-}" ]; }; then
  fail "input overrides require GOALRAIL_ARCHITECTURE_DRIFT_TESTING=1"
fi

[ -d "$root" ] || fail "repository root is unavailable: $root"
root=$(CDPATH= cd -- "$root" && pwd)

command -v jq >/dev/null 2>&1 || fail "jq is unavailable; run mise run setup"
command -v git >/dev/null 2>&1 || fail "git is unavailable"

work_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-architecture-drift.XXXXXX")

cleanup() {
  rm -rf "$work_root"
}
trap cleanup EXIT HUP INT TERM

metadata="$work_root/metadata.json"
public_api="$work_root/public-api.txt"
metadata_snapshot="$work_root/metadata-snapshot.json"
public_api_snapshot="$work_root/public-api.json"
source_files_jsonl="$work_root/source-files.jsonl"
source_files="$work_root/source-files.json"
package_records="$work_root/package-records.jsonl"
current="$work_root/current.json"

if [ -n "$metadata_override" ]; then
  [ -f "$metadata_override" ] || fail "Cargo metadata override is missing: $metadata_override"
  cp "$metadata_override" "$metadata"
elif ! cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" >"$metadata"; then
  fail "cargo metadata failed"
fi

if [ -n "$public_api_override" ]; then
  [ -f "$public_api_override" ] || fail "public API override is missing: $public_api_override"
  cp "$public_api_override" "$public_api"
elif ! "$script_dir/generate-public-api.sh" >"$public_api"; then
  fail "current rustdoc-visible public API is unavailable"
fi

if ! jq -e '
  .workspace_members as $workspace_ids
  | [.packages[] | select(.id as $id | $workspace_ids | index($id))] as $packages
  | ($packages | map(.name)) as $members
  | ($packages | map({
      name,
      root: (.manifest_path | rtrimstr("/Cargo.toml"))
    })) as $member_roots
  | {
      workspace_members: ($members | sort),
      owned_edges: ([
        $packages[] as $package
        | $package.dependencies[] as $dependency
        | select(
            $dependency.source == null
            and ($dependency.path | type == "string")
            and ($member_roots | any(
              .name == $dependency.name and .root == $dependency.path
            ))
          )
        | {
            from: $package.name,
            to: $dependency.name,
            kind: ($dependency.kind // "normal"),
            target: ($dependency.target // null),
            optional: $dependency.optional
          }
      ] | unique | sort_by(.from, .to, .kind, .target, .optional))
    }
' "$metadata" >"$metadata_snapshot"; then
  fail "Cargo metadata does not match format version 1"
fi

if ! jq -Rs 'split("\n") | map(select(length > 0)) | unique | sort' \
  "$public_api" >"$public_api_snapshot"; then
  fail "public API output could not be normalized"
fi

: >"$source_files_jsonl"
if ! jq -c '
  .workspace_members as $workspace_ids
  | .packages[]
  | select(.id as $id | $workspace_ids | index($id))
  | {package: .name, manifest_path: .manifest_path}
' "$metadata" >"$package_records"; then
  fail "workspace package inventory failed"
fi

package_index=0
while IFS= read -r package_record; do
  package=$(printf '%s\n' "$package_record" | jq -r '.package')
  manifest_path=$(printf '%s\n' "$package_record" | jq -r '.manifest_path')
  package_root=$(dirname -- "$manifest_path")
  case "$package_root/" in
    "$root/"*) ;;
    *) fail "workspace package is outside the repository: $manifest_path" ;;
  esac
  [ -d "$package_root" ] || fail "workspace package directory is missing: $package_root"

  package_index=$((package_index + 1))
  package_sources="$work_root/package-sources-$package_index.txt"
  package_sources_sorted="$work_root/package-sources-$package_index.sorted.txt"
  if ! find "$package_root" -type d -name target -prune -o -type f -name '*.rs' -print \
    >"$package_sources"; then
    fail "Rust source discovery failed for package: $package"
  fi
  LC_ALL=C sort "$package_sources" >"$package_sources_sorted"

  while IFS= read -r source_file; do
    relative_path=${source_file#"$root/"}
    line_count=$(awk 'END { print NR + 0 }' "$source_file")
    content_hash=$(git hash-object -- "$source_file")
    jq -cn \
      --arg path "$relative_path" \
      --arg package "$package" \
      --arg package_root "$package_root" \
      --argjson lines "$line_count" \
      --arg content_hash "$content_hash" \
      '{
        path: $path,
        package: $package,
        package_root: $package_root,
        lines: $lines,
        content_hash: $content_hash
      }' \
      >>"$source_files_jsonl"
  done <"$package_sources_sorted"
done <"$package_records"

if ! jq -s '
  sort_by(.path)
  | group_by(.path)
  | map(
      (map(.package_root | length) | max) as $deepest_root_length
      | [ .[] | select((.package_root | length) == $deepest_root_length) ] as $owners
      | if ($owners | length) != 1 then
          error("a Rust source path has ambiguous nearest package ownership")
        else
          $owners[0] | del(.package_root)
        end
    )
  | sort_by(.path)
' "$source_files_jsonl" >"$source_files"; then
  fail "workspace Rust source inventory could not be normalized"
fi

jq -n \
  --slurpfile metadata "$metadata_snapshot" \
  --slurpfile public_api "$public_api_snapshot" \
  --slurpfile source_files "$source_files" \
  '{
    schema_version: 1,
    workspace_members: $metadata[0].workspace_members,
    owned_edges: $metadata[0].owned_edges,
    public_api_items: $public_api[0],
    source_files: $source_files[0]
  }' >"$current"

if [ "$mode" = capture ]; then
  cat "$current"
  exit 0
fi

[ -f "$baseline" ] || fail "accepted baseline is missing: $baseline"
jq -e '
  . as $baseline
  | ($baseline | keys) == [
      "owned_edges",
      "public_api_items",
      "schema_version",
      "source_files",
      "workspace_members"
    ]
  and $baseline.schema_version == 1
  and ($baseline.workspace_members | type == "array")
  and all($baseline.workspace_members[]; type == "string" and length > 0)
  and ($baseline.workspace_members == ($baseline.workspace_members | unique | sort))
  and ($baseline.owned_edges | type == "array")
  and all($baseline.owned_edges[];
    . as $edge
    | ($edge | keys) == ["from", "kind", "optional", "target", "to"]
    and ($edge.from | type == "string" and length > 0)
    and ($edge.to | type == "string" and length > 0)
    and ($edge.kind | type == "string" and length > 0)
    and (($edge.target | type) == "null" or ($edge.target | type) == "string")
    and ($edge.optional | type == "boolean")
    and (($baseline.workspace_members | index($edge.from)) != null)
    and (($baseline.workspace_members | index($edge.to)) != null)
  )
  and ($baseline.owned_edges == (
    $baseline.owned_edges
    | unique
    | sort_by(.from, .to, .kind, .target, .optional)
  ))
  and ($baseline.public_api_items | type == "array")
  and all($baseline.public_api_items[]; type == "string" and length > 0)
  and ($baseline.public_api_items == ($baseline.public_api_items | unique | sort))
  and ($baseline.source_files | type == "array")
  and all($baseline.source_files[];
    . as $file
    | ($file | keys) == ["content_hash", "lines", "package", "path"]
    and ($file.path | type == "string" and length > 0)
    and ($file.path | startswith("/") | not)
    and ($file.path | endswith(".rs"))
    and ($file.path | split("/") | all(length > 0 and . != ".."))
    and ($file.package | type == "string" and length > 0)
    and (($baseline.workspace_members | index($file.package)) != null)
    and ($file.lines | type == "number")
    and $file.lines >= 0
    and $file.lines == ($file.lines | floor)
    and ($file.content_hash | type == "string")
    and ($file.content_hash | test("^[0-9a-f]{40}([0-9a-f]{24})?$"))
  )
  and ($baseline.source_files == ($baseline.source_files | sort_by(.path)))
  and (($baseline.source_files | map(.path) | length) ==
    ($baseline.source_files | map(.path) | unique | length))
' "$baseline" >/dev/null || fail "accepted baseline is invalid or noncanonical: $baseline"

case "$baseline" in
  "$root/"*) baseline_display=${baseline#"$root/"} ;;
  *) baseline_display=$baseline ;;
esac

jq -n \
  --arg baseline "$baseline_display" \
  --slurpfile before_input "$baseline" \
  --slurpfile current_input "$current" '
  def removed($before; $after):
    [$before[] as $item | select(($after | index($item)) == null) | $item];
  def added($before; $after): removed($after; $before);

  $before_input[0] as $before
  | $current_input[0] as $current
  | {
      workspace_members: {
        added: added($before.workspace_members; $current.workspace_members),
        removed: removed($before.workspace_members; $current.workspace_members)
      },
      owned_edges: {
        added: added($before.owned_edges; $current.owned_edges),
        removed: removed($before.owned_edges; $current.owned_edges)
      },
      public_api_items: {
        added: added($before.public_api_items; $current.public_api_items),
        removed: removed($before.public_api_items; $current.public_api_items)
      },
      source_files: {
        added: [
          $current.source_files[] as $file
          | select(($before.source_files | map(.path) | index($file.path)) == null)
          | {path: $file.path, package: $file.package, lines: $file.lines}
        ],
        removed: [
          $before.source_files[] as $file
          | select(($current.source_files | map(.path) | index($file.path)) == null)
          | {path: $file.path, package: $file.package, lines: $file.lines}
        ],
        changed: [
          $current.source_files[] as $after
          | $before.source_files[] as $prior
          | select(
              $prior.path == $after.path
              and (
                $prior.package != $after.package
                or $prior.lines != $after.lines
                or $prior.content_hash != $after.content_hash
              )
            )
          | {
              path: $after.path,
              before: {package: $prior.package, lines: $prior.lines},
              after: {package: $after.package, lines: $after.lines},
              line_delta: ($after.lines - $prior.lines)
            }
        ]
      }
    } as $changes
  | ([
      $changes.workspace_members.added,
      $changes.workspace_members.removed,
      $changes.owned_edges.added,
      $changes.owned_edges.removed,
      $changes.public_api_items.added,
      $changes.public_api_items.removed,
      $changes.source_files.added,
      $changes.source_files.removed,
      $changes.source_files.changed
    ] | map(length) | add) as $change_count
  | {
      schema_version: 1,
      advisory: true,
      verdict: (if $change_count == 0 then "NO_CHANGE" else "REVIEW" end),
      baseline: $baseline,
      change_count: $change_count,
      current_summary: {
        workspace_members: ($current.workspace_members | length),
        owned_edges: ($current.owned_edges | length),
        public_api_items: ($current.public_api_items | length),
        rust_source_files: ($current.source_files | length),
        rust_source_lines: ([$current.source_files[].lines] | add // 0),
        largest_files: (
          $current.source_files
          | sort_by(.lines, .path)
          | reverse
          | .[:5]
          | map({path, package, lines})
        )
      },
      changes: $changes,
      limitations: [
        "REVIEW is an advisory drift signal, not an architecture violation",
        "Cargo metadata covers workspace package edges, not intra-crate dependencies",
        "public API items cover only the pinned cargo-public-api trial surface",
        "source content changes identify review scope but do not classify semantic responsibility"
      ]
    }
'

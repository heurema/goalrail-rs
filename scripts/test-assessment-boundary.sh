#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
checker="$repo_root/scripts/check-assessment-boundary.sh"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-assessment-boundary-test.XXXXXX")

cleanup() {
  rm -rf "$test_root"
}
trap cleanup EXIT HUP INT TERM

cd "$repo_root"

metadata="$test_root/metadata.json"
cargo metadata --format-version 1 --no-deps >"$metadata"

GOALRAIL_ASSESSMENT_GATE_TESTING=1 \
  GOALRAIL_CARGO_METADATA_JSON="$metadata" \
  "$checker" >/dev/null

override_error="$test_root/override-error.txt"
if GOALRAIL_CARGO_METADATA_JSON="$metadata" "$checker" >/dev/null 2>"$override_error"; then
  echo "assessment boundary accepted a test override without the testing guard" >&2
  exit 1
fi
grep -F "test overrides require GOALRAIL_ASSESSMENT_GATE_TESTING=1" "$override_error" >/dev/null || {
  echo "assessment boundary rejected an unguarded override for an unexpected reason" >&2
  exit 1
}

reverse_edge_metadata="$test_root/reverse-edge.json"
jq '
  (.packages[] | select(.name == "gr-skill-assessment").dependencies) += [{
    "name": "gr-inspect-codex",
    "source": null,
    "req": "*",
    "kind": null,
    "rename": null,
    "optional": false,
    "uses_default_features": true,
    "features": [],
    "target": null,
    "registry": null,
    "path": "crates/gr-inspect-codex"
  }]
' "$metadata" >"$reverse_edge_metadata"

if GOALRAIL_ASSESSMENT_GATE_TESTING=1 \
  GOALRAIL_CARGO_METADATA_JSON="$reverse_edge_metadata" \
  "$checker" >/dev/null 2>&1; then
  echo "assessment boundary accepted a reverse owned dependency" >&2
  exit 1
fi

extra_consumer_metadata="$test_root/extra-consumer.json"
jq '
  (.packages[] | select(.name == "gr").dependencies) += [{
    "name": "gr-skill-assessment",
    "source": null,
    "req": "*",
    "kind": null,
    "rename": null,
    "optional": false,
    "uses_default_features": true,
    "features": [],
    "target": null,
    "registry": null,
    "path": "crates/gr-skill-assessment"
  }]
' "$metadata" >"$extra_consumer_metadata"

if GOALRAIL_ASSESSMENT_GATE_TESTING=1 \
  GOALRAIL_CARGO_METADATA_JSON="$extra_consumer_metadata" \
  "$checker" >/dev/null 2>&1; then
  echo "assessment boundary accepted an extra owned consumer" >&2
  exit 1
fi

feature_metadata="$test_root/feature.json"
jq '
  (.packages[] | select(.name == "gr-skill-assessment").features) = {
    "std": []
  }
' "$metadata" >"$feature_metadata"

if GOALRAIL_ASSESSMENT_GATE_TESTING=1 \
  GOALRAIL_CARGO_METADATA_JSON="$feature_metadata" \
  "$checker" >/dev/null 2>&1; then
  echo "assessment boundary accepted a feature-gated capability path" >&2
  exit 1
fi

assessment_crate="$test_root/gr-skill-assessment"
cp -R crates/gr-skill-assessment "$assessment_crate"
cp Cargo.lock "$assessment_crate/Cargo.lock"
printf '\nextern /* bypass */ crate std;\n' >>"$assessment_crate/src/lib.rs"

if GOALRAIL_ASSESSMENT_GATE_TESTING=1 \
  GOALRAIL_CARGO_METADATA_JSON="$metadata" \
  GOALRAIL_ASSESSMENT_MANIFEST="$assessment_crate/Cargo.toml" \
  "$checker" >/dev/null 2>&1; then
  echo "assessment boundary accepted a token-obfuscated std opt-in" >&2
  exit 1
fi

echo "ASSESSMENT_BOUNDARY_TEST_OK sabotage=unguarded-override,reverse-edge,extra-consumer,feature,commented-std"

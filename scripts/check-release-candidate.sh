#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v shellcheck >/dev/null 2>&1 || {
  echo "check-release-candidate: shellcheck is unavailable" >&2
  exit 1
}

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
./scripts/check-assessment-boundary.sh
./scripts/test-assessment-boundary.sh
./scripts/test-goalrail-plugin.sh
./scripts/test-homebrew-update-state.sh
./scripts/test-plugin-update-target.sh
./scripts/test-release-tooling.sh
./scripts/test-homebrew-promotion.sh

shellcheck \
  scripts/prepare-release.sh \
  scripts/package-release.sh \
  scripts/check-release.sh \
  scripts/assemble-release.sh \
  scripts/check-release-bundle.sh \
  scripts/check-github-release.sh \
  scripts/check-github-run.sh \
  scripts/check-release-candidate.sh \
  scripts/check-remote-release-tag.sh \
  scripts/check-remote-release-tag-absent.sh \
  scripts/smoke-release-binary.sh \
  scripts/promote-homebrew.sh \
  scripts/release-preflight.sh \
  scripts/test-release-tooling.sh \
  scripts/test-homebrew-promotion.sh \
  scripts/test-homebrew-update-state.sh \
  scripts/test-plugin-update-target.sh \
  plugins/goalrail/skills/goalrail/scripts/homebrew-update-state.sh \
  plugins/goalrail/skills/goalrail/scripts/plugin-update-target.sh

echo "RELEASE_CANDIDATE_SOURCE_OK"

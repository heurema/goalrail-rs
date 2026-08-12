#!/bin/sh

set -eu

fail() {
  echo "release-tooling-test: $1" >&2
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

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
command -v file >/dev/null 2>&1 || fail "file is unavailable"
command -v ruby >/dev/null 2>&1 || fail "ruby is unavailable"
[ "$(git check-attr text -- LICENSE)" = "LICENSE: text: set" ] ||
  fail "LICENSE must be tracked as text"
[ "$(git check-attr eol -- LICENSE)" = "LICENSE: eol: lf" ] ||
  fail "LICENSE must use LF in every checkout"
[ "$(git check-attr text -- Cargo.lock)" = "Cargo.lock: text: set" ] ||
  fail "Cargo.lock must be tracked as text"
[ "$(git check-attr eol -- Cargo.lock)" = "Cargo.lock: eol: lf" ] ||
  fail "Cargo.lock must use LF in every checkout"

check_workflow_contract() {
  workflow_file=$1

  grep -F 'workflow_dispatch:' "$workflow_file" >/dev/null || return 1
  if grep -Eq '^[[:space:]]+push:|contents:[[:space:]]*write|gh release|softprops/action-gh-release' \
    "$workflow_file"; then
    return 1
  fi
  if grep -F 'inputs.tag' "$workflow_file" >/dev/null; then
    return 1
  fi
  # shellcheck disable=SC2016
  for marker in \
    'runner: macos-15' \
    'target: aarch64-apple-darwin' \
    'runner: ubuntu-22.04' \
    'target: x86_64-unknown-linux-gnu' \
    'runner: windows-2025' \
    'target: x86_64-pc-windows-msvc' \
    'persist-credentials: false' \
    'WORKFLOW_COMMIT: ${{ github.sha }}' \
    'ref: ${{ inputs.source_commit }}' \
    '[[ "${WORKFLOW_COMMIT}" == "${REQUESTED_COMMIT}" ]]' \
    '[[ "${source_commit}" == "${REQUESTED_COMMIT}" ]]' \
    './scripts/check-release-candidate.sh' \
    'goalrail-${{ needs.resolve.outputs.tag }}-release-candidate' \
    '"${GITHUB_RUN_ID}" "${GITHUB_RUN_ATTEMPT}"' \
    '"${GITHUB_WORKFLOW}" "${GITHUB_EVENT_NAME}" "${GITHUB_REF_NAME}" dist' \
    'READY_FOR_TAG' \
    'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1' \
    'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a' \
    'actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c'; do
    grep -F "$marker" "$workflow_file" >/dev/null || return 1
  done
  [ "$(grep -Fc './scripts/check-remote-release-tag-absent.sh' "$workflow_file")" -eq 2 ] ||
    return 1
  # shellcheck disable=SC2016
  [ "$(grep -Fc '[[ "$(git rev-parse HEAD)" == "${SOURCE_COMMIT}" ]]' \
    "$workflow_file")" -eq 3 ] || return 1
}

check_release_runbook_contract() {
  runbook_file=$1

  # shellcheck disable=SC2016
  grep -F 'git push origin "${source_commit}:refs/heads/${candidate_branch}"' \
    "$runbook_file" >/dev/null || return 1
  # shellcheck disable=SC2016
  grep -F 'git push origin "${source_commit}:refs/heads/main"' \
    "$runbook_file" >/dev/null || return 1
  if grep -Eq '"\$[[:alpha:]_][[:alnum:]_]*:' "$runbook_file"; then
    return 1
  fi
}

workflow=.github/workflows/release.yml
check_workflow_contract "$workflow" || fail "release workflow contract is incomplete"
release_runbook=docs/release.md
check_release_runbook_contract "$release_runbook" ||
  fail "release runbook contains a shell-ambiguous refspec"

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
  scripts/smoke-release-binary.sh \
  scripts/promote-homebrew.sh \
  scripts/release-preflight.sh; do
  sh -n "$script"
done

package_id=$(cargo pkgid -p gr)
version=${package_id##*@}
source_commit=$(git rev-parse HEAD)
lock_sha256=$(checksum Cargo.lock)
rustc_release=$(rustc -vV | awk '/^release:/ { print $2 }')
output_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-release-test.XXXXXX")
fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-release-fixture.XXXXXX")

cleanup() {
  rm -rf "$output_root" "$fixture_root"
}
trap cleanup EXIT HUP INT TERM

runbook_refspec_mutant="$fixture_root/release-runbook-unsafe-refspec.md"
# shellcheck disable=SC2016
sed 's|"${source_commit}:refs/heads/${candidate_branch}"|"$source_commit:refs/heads/$candidate_branch"|' \
  "$release_runbook" >"$runbook_refspec_mutant"
if cmp -s "$release_runbook" "$runbook_refspec_mutant"; then
  fail "release runbook refspec sabotage did not change the fixture"
fi
if check_release_runbook_contract "$runbook_refspec_mutant"; then
  fail "release runbook contract accepted a zsh-ambiguous refspec"
fi
runbook_main_refspec_mutant="$fixture_root/release-runbook-unsafe-main-refspec.md"
# shellcheck disable=SC2016
sed 's|"${source_commit}:refs/heads/main"|"$source_commit:refs/heads/main"|' \
  "$release_runbook" >"$runbook_main_refspec_mutant"
if cmp -s "$release_runbook" "$runbook_main_refspec_mutant"; then
  fail "release runbook main-refspec sabotage did not change the fixture"
fi
if check_release_runbook_contract "$runbook_main_refspec_mutant"; then
  fail "release runbook contract accepted a zsh-ambiguous main refspec"
fi

workflow_sha_mutant="$fixture_root/workflow-without-sha-binding.yml"
# shellcheck disable=SC2016
sed '/\[\[ "${WORKFLOW_COMMIT}" == "${REQUESTED_COMMIT}" \]\]/d' \
  "$workflow" >"$workflow_sha_mutant"
if check_workflow_contract "$workflow_sha_mutant"; then
  fail "workflow contract accepted a missing dispatch SHA binding"
fi

workflow_tag_mutant="$fixture_root/workflow-without-final-tag-check.yml"
awk '
  /check-remote-release-tag-absent\.sh/ {
    count += 1
    if (count == 2) next
  }
  { print }
' "$workflow" >"$workflow_tag_mutant"
if check_workflow_contract "$workflow_tag_mutant"; then
  fail "workflow contract accepted a missing final tag-absence check"
fi

line_endings_repo="$fixture_root/line-endings-repo"
line_endings_checkout="$fixture_root/line-endings-checkout"
mkdir -p "$line_endings_repo"
cp .gitattributes LICENSE Cargo.lock "$line_endings_repo/"
git -C "$line_endings_repo" init -q
git -C "$line_endings_repo" config user.name fixture
git -C "$line_endings_repo" config user.email fixture@example.invalid
git -C "$line_endings_repo" add .
git -C "$line_endings_repo" commit -q -m line-endings
git clone -q -c core.autocrlf=true \
  "$line_endings_repo" "$line_endings_checkout"
cmp -s LICENSE "$line_endings_checkout/LICENSE" ||
  fail "core.autocrlf changed LICENSE bytes"
cmp -s Cargo.lock "$line_endings_checkout/Cargo.lock" ||
  fail "core.autocrlf changed Cargo.lock bytes"

system=$(uname -s)
machine=$(uname -m)
case "$system:$machine" in
  Darwin:arm64) host_target=aarch64-apple-darwin ;;
  Linux:x86_64) host_target=x86_64-unknown-linux-gnu ;;
  MINGW*:x86_64|MSYS*:x86_64|CYGWIN*:x86_64)
    host_target=x86_64-pc-windows-msvc
    ;;
  *) fail "unsupported test host: $system $machine" ;;
esac

scripts/prepare-release.sh "$version" "$host_target" "$output_root/native" >/dev/null
scripts/check-release.sh "$version" "$host_target" "$output_root/native" >/dev/null

if scripts/prepare-release.sh "$version-dev" "$host_target" "$output_root/invalid" >/dev/null 2>&1; then
  fail "release tooling accepted a non-stable version"
fi

case "$host_target" in
  aarch64-apple-darwin) wrong_target=x86_64-unknown-linux-gnu ;;
  *) wrong_target=aarch64-apple-darwin ;;
esac
if scripts/prepare-release.sh "$version" "$wrong_target" "$output_root/wrong-host" >/dev/null 2>&1; then
  fail "release tooling accepted a non-native target"
fi

write_bytes() {
  output=$1
  offset=$2
  bytes=$3
  printf '%b' "$bytes" | dd of="$output" bs=1 seek="$offset" conv=notrunc 2>/dev/null
}

write_binary_fixture() {
  target=$1
  output=$2

  case "$target" in
    aarch64-apple-darwin)
      dd if=/dev/zero of="$output" bs=1 count=32 2>/dev/null
      write_bytes "$output" 0 '\317\372\355\376\014\000\000\001'
      write_bytes "$output" 12 '\002\000\000\000'
      ;;
    x86_64-unknown-linux-gnu)
      dd if=/dev/zero of="$output" bs=1 count=64 2>/dev/null
      write_bytes "$output" 0 '\177ELF\002\001\001\000'
      write_bytes "$output" 16 '\002\000\076\000\001\000\000\000'
      write_bytes "$output" 52 '\100\000'
      ;;
    x86_64-pc-windows-msvc)
      dd if=/dev/zero of="$output" bs=1 count=512 2>/dev/null
      write_bytes "$output" 0 'MZ'
      write_bytes "$output" 60 '\200\000\000\000'
      write_bytes "$output" 128 'PE\000\000'
      write_bytes "$output" 132 '\144\206'
      write_bytes "$output" 148 '\360\000\002\000'
      write_bytes "$output" 152 '\013\002'
      ;;
    *) fail "unsupported fixture target: $target" ;;
  esac
  chmod 0755 "$output"
}

write_target_fixture() {
  target=$1
  release_dir=$2
  payload_dir="$fixture_root/payload-$target"
  mkdir -p "$payload_dir" "$release_dir"

  case "$target" in
    x86_64-pc-windows-msvc) binary_name=gr.exe ;;
    *) binary_name=gr ;;
  esac
  binary="$payload_dir/$binary_name"
  write_binary_fixture "$target" "$binary"
  cp LICENSE "$payload_dir/LICENSE"

  tag="v$version"
  artifact="goalrail-$tag-$target.tar.gz"
  archive="$release_dir/$artifact"
  manifest="$release_dir/goalrail-$tag-$target.json"
  COPYFILE_DISABLE=1 tar -C "$payload_dir" -czf "$archive" LICENSE "$binary_name"
  archive_sha256=$(checksum "$archive")
  archive_size=$(wc -c <"$archive" | tr -d ' ')
  printf '%s  %s\n' "$archive_sha256" "$artifact" >"$archive.sha256"

  jq -n \
    --arg version "$version" \
    --arg tag "$tag" \
    --arg target "$target" \
    --arg binary "$binary_name" \
    --arg artifact "$artifact" \
    --arg sha256 "$archive_sha256" \
    --arg source_commit "$source_commit" \
    --arg lock_sha256 "$lock_sha256" \
    --arg rustc_release "$rustc_release" \
    --argjson archive_size "$archive_size" \
    '{
      schemaVersion: 1,
      product: "goalrail",
      version: $version,
      tag: $tag,
      target: $target,
      binary: $binary,
      binaryVersion: ("gr " + $version),
      license: "MIT",
      artifact: $artifact,
      artifactSizeBytes: $archive_size,
      sha256: $sha256,
      checksumArtifact: ($artifact + ".sha256"),
      downloadUrl: ("https://github.com/heurema/goalrail-rs/releases/download/" + $tag + "/" + $artifact),
      build: {
        sourceCommit: $source_commit,
        cargoLockSha256: $lock_sha256,
        rustcRelease: $rustc_release,
        rustcHost: $target,
        runnerImage: "fixture",
        runnerImageVersion: "1"
      }
    }' >"$manifest"
}

release_dir="$output_root/bundle/v$version"
for target in \
  aarch64-apple-darwin \
  x86_64-unknown-linux-gnu \
  x86_64-pc-windows-msvc; do
  write_target_fixture "$target" "$release_dir"
  scripts/check-release.sh "$version" "$target" "$output_root/bundle" >/dev/null
done

scripts/assemble-release.sh \
  "$version" "$source_commit" 123456789 2 \
  'build release candidate' workflow_dispatch "release-candidate/v$version" \
  "$output_root/bundle" >/dev/null
scripts/check-release-bundle.sh \
  "$version" "$source_commit" 123456789 2 "$output_root/bundle" >/dev/null

fake_file_bin="$fixture_root/fake-file-bin"
mkdir -p "$fake_file_bin"
cat >"$fake_file_bin/file" <<'EOF'
#!/bin/sh
printf '%s\n' "${GOALRAIL_TEST_FILE_DESCRIPTION:?}"
EOF
chmod 0755 "$fake_file_bin/file"
GOALRAIL_TEST_FILE_DESCRIPTION='Mach-O 64-bit arm64 executable' \
  PATH="$fake_file_bin:$PATH" \
  scripts/check-release.sh \
  "$version" aarch64-apple-darwin "$output_root/bundle" >/dev/null
if GOALRAIL_TEST_FILE_DESCRIPTION='Mach-O 64-bit x86_64 executable' \
  PATH="$fake_file_bin:$PATH" \
  scripts/check-release.sh \
  "$version" aarch64-apple-darwin "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted the wrong Mach-O architecture"
fi

manifest="$release_dir/goalrail-v$version-x86_64-unknown-linux-gnu.json"
manifest_clean="$manifest.clean"
cp "$manifest" "$manifest_clean"
jq '.product = "wrong"' "$manifest_clean" >"$manifest"
if scripts/check-release.sh \
  "$version" x86_64-unknown-linux-gnu "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a mismatched target manifest"
fi
mv "$manifest_clean" "$manifest"

manifest_clean="$manifest.clean"
cp "$manifest" "$manifest_clean"
jq '.build.rustcHost = "aarch64-apple-darwin"' "$manifest_clean" >"$manifest"
if scripts/check-release.sh \
  "$version" x86_64-unknown-linux-gnu "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a mismatched rustc host"
fi
mv "$manifest_clean" "$manifest"

archive="$release_dir/goalrail-v$version-x86_64-pc-windows-msvc.tar.gz"
archive_clean="$archive.clean"
cp "$archive" "$archive_clean"
printf 'tampered' >>"$archive"
if scripts/check-release.sh \
  "$version" x86_64-pc-windows-msvc "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a tampered archive"
fi
mv "$archive_clean" "$archive"

invalid_root="$output_root/invalid-format"
cp -R "$output_root/bundle" "$invalid_root"
invalid_release_dir="$invalid_root/v$version"
invalid_archive="$invalid_release_dir/goalrail-v$version-x86_64-unknown-linux-gnu.tar.gz"
invalid_payload="$fixture_root/invalid-format"
mkdir -p "$invalid_payload"
cp LICENSE "$invalid_payload/LICENSE"
cat >"$invalid_payload/gr" <<EOF
#!/bin/sh
printf '%s\\n' 'gr $version'
EOF
chmod 0755 "$invalid_payload/gr"
COPYFILE_DISABLE=1 tar -C "$invalid_payload" -czf "$invalid_archive" LICENSE gr
invalid_sha256=$(checksum "$invalid_archive")
invalid_size=$(wc -c <"$invalid_archive" | tr -d ' ')
printf '%s  %s\n' "$invalid_sha256" "${invalid_archive##*/}" >"$invalid_archive.sha256"
invalid_manifest="$invalid_release_dir/goalrail-v$version-x86_64-unknown-linux-gnu.json"
jq --arg sha256 "$invalid_sha256" --argjson size "$invalid_size" \
  '.sha256 = $sha256 | .artifactSizeBytes = $size' \
  "$invalid_manifest" >"$invalid_manifest.updated"
mv "$invalid_manifest.updated" "$invalid_manifest"
if scripts/check-release.sh \
  "$version" x86_64-unknown-linux-gnu "$invalid_root" >/dev/null 2>&1; then
  fail "release tooling accepted a shell script as a Linux binary"
fi

release_manifest="$release_dir/release.json"
release_manifest_clean="$release_manifest.clean"
cp "$release_manifest" "$release_manifest_clean"
jq '.sourceCommit = "0000000000000000000000000000000000000000"' \
  "$release_manifest_clean" >"$release_manifest"
if scripts/check-release-bundle.sh \
  "$version" "$source_commit" 123456789 2 \
  "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a mismatched aggregate manifest"
fi
mv "$release_manifest_clean" "$release_manifest"

release_manifest_clean="$release_manifest.clean"
cp "$release_manifest" "$release_manifest_clean"
jq '.run.id = "987654321"' "$release_manifest_clean" >"$release_manifest"
if scripts/check-release-bundle.sh \
  "$version" "$source_commit" 123456789 2 \
  "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a mismatched workflow run ID"
fi
mv "$release_manifest_clean" "$release_manifest"

formula="$release_dir/goalrail.rb"
formula_clean="$formula.clean"
cp "$formula" "$formula_clean"
awk -v version="$version" '
  { print }
  /^[[:space:]]+sha256 / { printf "  version(\"%s\")\n", version }
' "$formula_clean" >"$formula"
if scripts/check-release-bundle.sh \
  "$version" "$source_commit" 123456789 2 \
  "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted an explicit formula version"
fi
mv "$formula_clean" "$formula"

release_response="$fixture_root/release-response.json"
jq -n --arg tag "v$version" '{
  tagName: $tag,
  isDraft: true,
  isPrerelease: false,
  assets: []
}' >"$release_response"
for asset in \
  "goalrail-v$version-aarch64-apple-darwin.tar.gz" \
  "goalrail-v$version-aarch64-apple-darwin.tar.gz.sha256" \
  "goalrail-v$version-aarch64-apple-darwin.json" \
  "goalrail-v$version-x86_64-unknown-linux-gnu.tar.gz" \
  "goalrail-v$version-x86_64-unknown-linux-gnu.tar.gz.sha256" \
  "goalrail-v$version-x86_64-unknown-linux-gnu.json" \
  "goalrail-v$version-x86_64-pc-windows-msvc.tar.gz" \
  "goalrail-v$version-x86_64-pc-windows-msvc.tar.gz.sha256" \
  "goalrail-v$version-x86_64-pc-windows-msvc.json" \
  goalrail.rb \
  release.json; do
  asset_file="$release_dir/$asset"
  asset_digest="sha256:$(checksum "$asset_file")"
  asset_size=$(wc -c <"$asset_file" | tr -d ' ')
  jq \
    --arg name "$asset" \
    --arg digest "$asset_digest" \
    --argjson size "$asset_size" \
    '.assets += [{name: $name, state: "uploaded", digest: $digest, size: $size}]' \
    "$release_response" >"$release_response.next"
  mv "$release_response.next" "$release_response"
done

fake_gh_bin="$fixture_root/fake-gh-bin"
mkdir -p "$fake_gh_bin"
cat >"$fake_gh_bin/gh" <<'EOF'
#!/bin/sh
set -eu
case "$1:$2" in
  api:*) cat "${GOALRAIL_TEST_GH_API_RESPONSE:?}" ;;
  release:view) cat "${GOALRAIL_TEST_GH_RESPONSE:?}" ;;
  *) exit 1 ;;
esac
EOF
chmod 0755 "$fake_gh_bin/gh"

run_response="$fixture_root/run-response.json"
jq -n \
  --argjson id 123456789 \
  --arg source_commit "$source_commit" \
  --arg head_branch "release-candidate/v$version" '{
    id: $id,
    name: "build release candidate",
    display_title: "build release bundle",
    path: ".github/workflows/release.yml",
    event: "workflow_dispatch",
    head_branch: $head_branch,
    head_sha: $source_commit,
    status: "completed",
    conclusion: "success",
    run_attempt: 2,
    html_url: "https://example.invalid/actions/runs/123456789",
    repository: {full_name: "heurema/goalrail-rs"}
  }' >"$run_response"
run_output=$(PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_API_RESPONSE="$run_response" \
  scripts/check-github-run.sh \
    123456789 "$source_commit" "release-candidate/v$version")
printf '%s\n' "$run_output" | jq -e \
  --arg source_commit "$source_commit" \
  --arg candidate_branch "release-candidate/v$version" '
    .verdict == "RUN_VERIFIED"
    and .runId == "123456789"
    and .runAttempt == "2"
    and .sourceCommit == $source_commit
    and .candidateBranch == $candidate_branch
  ' >/dev/null || fail "GitHub run checker omitted exact run identity"

jq '.name = "build release bundle"' \
  "$run_response" >"$run_response.registry-name"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_API_RESPONSE="$run_response.registry-name" \
  scripts/check-github-run.sh \
    123456789 "$source_commit" "release-candidate/v$version" >/dev/null 2>&1; then
  fail "GitHub run checker accepted the default-branch registry name as run identity"
fi

jq '.head_sha = "0000000000000000000000000000000000000000"' \
  "$run_response" >"$run_response.wrong-sha"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_API_RESPONSE="$run_response.wrong-sha" \
  scripts/check-github-run.sh \
    123456789 "$source_commit" "release-candidate/v$version" >/dev/null 2>&1; then
  fail "GitHub run checker accepted a mismatched source commit"
fi

jq '.status = "in_progress" | .conclusion = null' \
  "$run_response" >"$run_response.in-progress"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_API_RESPONSE="$run_response.in-progress" \
  scripts/check-github-run.sh \
    123456789 "$source_commit" "release-candidate/v$version" >/dev/null 2>&1; then
  fail "GitHub run checker accepted an incomplete run"
fi

public_work="$fixture_root/public-work"
public_remote="$fixture_root/public-remote.git"
git clone -q --no-tags . "$public_work"
git init -q --bare "$public_remote"
git -C "$public_work" config user.name fixture
git -C "$public_work" config user.email fixture@example.invalid
git -C "$public_work" tag -a "v$version" "$source_commit" -m release
git -C "$public_work" push -q "$public_remote" "refs/tags/v$version"
PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response" \
  scripts/check-github-release.sh \
    "$version" draft 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null

if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response" \
  scripts/check-github-release.sh \
    "$version" draft 987654321 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "GitHub Release checker accepted a different selected run ID"
fi

jq 'del(.assets[0])' "$release_response" >"$release_response.partial"
partial_output=$(PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.partial" \
  scripts/check-github-release.sh \
    "$version" draft-partial 123456789 2 \
    "$output_root/bundle" "$public_remote")
printf '%s\n' "$partial_output" | jq -e \
  --arg missing "goalrail-v$version-aarch64-apple-darwin.tar.gz" '
    .verdict == "DRAFT_PARTIAL_VERIFIED"
    and .runId == "123456789"
    and .runAttempt == "2"
    and .missing == [$missing]
  ' >/dev/null || fail "partial draft checker did not report the exact missing asset"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.partial" \
  scripts/check-github-release.sh \
    "$version" draft 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "complete draft checker accepted a missing public asset"
fi

jq '.isDraft = false' "$release_response" >"$release_response.published"
PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.published" \
  scripts/check-github-release.sh \
    "$version" published 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null

jq '.assets += [{
  name: "unexpected.txt",
  state: "uploaded",
  digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  size: 0
}]' "$release_response" >"$release_response.extra"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.extra" \
  scripts/check-github-release.sh \
    "$version" draft 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "GitHub Release checker accepted an unexpected public asset"
fi

jq '(.assets[0].digest) = "sha256:0000000000000000000000000000000000000000000000000000000000000000"' \
  "$release_response" >"$release_response.tampered"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.tampered" \
  scripts/check-github-release.sh \
    "$version" draft 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "GitHub Release checker accepted a mismatched public digest"
fi
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response.tampered" \
  scripts/check-github-release.sh \
    "$version" draft-partial 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "partial draft checker accepted a mismatched existing digest"
fi

git -C "$public_work" tag -f -a "v$version" HEAD^ -m moved >/dev/null
git -C "$public_work" push -q --force "$public_remote" "refs/tags/v$version"
if PATH="$fake_gh_bin:$PATH" \
  GOALRAIL_TEST_GH_RESPONSE="$release_response" \
  scripts/check-github-release.sh \
    "$version" draft 123456789 2 \
    "$output_root/bundle" "$public_remote" >/dev/null 2>&1; then
  fail "GitHub Release checker accepted a moved remote release tag"
fi

remote_repo="$fixture_root/remote.git"
remote_work="$fixture_root/remote-work"
git init -q --bare "$remote_repo"
git init -q "$remote_work"
git -C "$remote_work" config user.name fixture
git -C "$remote_work" config user.email fixture@example.invalid
git -C "$remote_work" commit -q --allow-empty -m first
remote_commit=$(git -C "$remote_work" rev-parse HEAD)
scripts/check-remote-release-tag-absent.sh v1.2.3 "$remote_repo" >/dev/null
git -C "$remote_work" tag -a v1.2.3 -m release
git -C "$remote_work" push -q "$remote_repo" HEAD:refs/heads/main refs/tags/v1.2.3
if scripts/check-remote-release-tag-absent.sh \
  v1.2.3 "$remote_repo" >/dev/null 2>&1; then
  fail "remote tag absence checker accepted an existing tag"
fi
scripts/check-remote-release-tag.sh v1.2.3 "$remote_commit" "$remote_repo" >/dev/null
git -C "$remote_work" commit -q --allow-empty -m second
git -C "$remote_work" tag -f -a v1.2.3 -m moved >/dev/null
git -C "$remote_work" push -q --force "$remote_repo" refs/tags/v1.2.3
if scripts/check-remote-release-tag.sh \
  v1.2.3 "$remote_commit" "$remote_repo" >/dev/null 2>&1; then
  fail "remote release tag checker accepted a moved tag"
fi

preflight_root="$fixture_root/preflight"
mkdir -p \
  "$preflight_root/.github/workflows" \
  "$preflight_root/release/homebrew" \
  "$preflight_root/scripts" \
  "$preflight_root/src"
cp scripts/release-preflight.sh "$preflight_root/scripts/release-preflight.sh"
cp release/homebrew/goalrail.rb.in "$preflight_root/release/homebrew/goalrail.rb.in"
cp LICENSE "$preflight_root/LICENSE"
cp .github/workflows/release.yml "$preflight_root/.github/workflows/release.yml"
for required in \
  check-release-candidate.sh \
  check-github-release.sh \
  check-github-run.sh \
  check-remote-release-tag-absent.sh \
  prepare-release.sh \
  package-release.sh \
  check-release.sh \
  assemble-release.sh \
  check-release-bundle.sh \
  check-remote-release-tag.sh \
  promote-homebrew.sh \
  smoke-release-binary.sh; do
  printf '#!/bin/sh\nexit 0\n' >"$preflight_root/scripts/$required"
  chmod 0755 "$preflight_root/scripts/$required"
done
cat >"$preflight_root/Cargo.toml" <<'EOF'
[package]
name = "gr"
version = "1.2.3"
edition = "2024"
EOF
printf 'fn main() {}\n' >"$preflight_root/src/main.rs"
cargo generate-lockfile --quiet --offline --manifest-path "$preflight_root/Cargo.toml"
git -C "$preflight_root" init -q
git -C "$preflight_root" config user.name fixture
git -C "$preflight_root" config user.email fixture@example.invalid
git -C "$preflight_root" add .
git -C "$preflight_root" commit -q -m candidate
preflight_remote="$fixture_root/preflight-remote.git"
git init -q --bare "$preflight_remote"
git -C "$preflight_root" remote add origin "$preflight_remote"
preflight_output=$("$preflight_root/scripts/release-preflight.sh" 1.2.3)
printf '%s\n' "$preflight_output" | jq -e '
  .schemaVersion == 3
  and .verdict == "READY_FOR_CANDIDATE"
  and .version == "1.2.3"
  and .plannedTag == "v1.2.3"
  and (.sourceCommit | test("^[0-9a-f]{40}$"))
' >/dev/null || fail "preflight did not accept a clean untagged candidate"

printf 'dirty\n' >>"$preflight_root/LICENSE"
if dirty_output=$("$preflight_root/scripts/release-preflight.sh" 1.2.3); then
  fail "preflight accepted a dirty candidate"
else
  dirty_status=$?
fi
[ "$dirty_status" -eq 4 ] || fail "dirty preflight returned an unexpected exit code"
printf '%s\n' "$dirty_output" | jq -e '
  .verdict == "BLOCKED"
  and any(.findings[]; .code == "WORKTREE_DIRTY")
' >/dev/null || fail "dirty preflight omitted WORKTREE_DIRTY"
git -C "$preflight_root" restore LICENSE

git -C "$preflight_root" tag -a v1.2.3 -m release
if tagged_output=$("$preflight_root/scripts/release-preflight.sh" 1.2.3); then
  fail "preflight accepted an existing candidate tag"
else
  tagged_status=$?
fi
[ "$tagged_status" -eq 4 ] || fail "tagged preflight returned an unexpected exit code"
printf '%s\n' "$tagged_output" | jq -e '
  .verdict == "BLOCKED"
  and any(.findings[]; .code == "LOCAL_TAG_EXISTS")
' >/dev/null || fail "tagged preflight omitted LOCAL_TAG_EXISTS"
git -C "$preflight_root" tag -d v1.2.3 >/dev/null

git -C "$preflight_root" tag -a v1.2.3 -m release
git -C "$preflight_root" push -q origin refs/tags/v1.2.3
git -C "$preflight_root" tag -d v1.2.3 >/dev/null
if remote_tagged_output=$("$preflight_root/scripts/release-preflight.sh" 1.2.3); then
  fail "preflight accepted an existing remote candidate tag"
else
  remote_tagged_status=$?
fi
[ "$remote_tagged_status" -eq 4 ] ||
  fail "remote-tagged preflight returned an unexpected exit code"
printf '%s\n' "$remote_tagged_output" | jq -e '
  .verdict == "BLOCKED"
  and any(.findings[]; .code == "REMOTE_TAG_EXISTS")
' >/dev/null || fail "tagged preflight omitted REMOTE_TAG_EXISTS"

git -C "$preflight_root" remote set-url origin "$fixture_root/missing-remote.git"
if unavailable_remote_output=$("$preflight_root/scripts/release-preflight.sh" 1.2.3); then
  fail "preflight accepted an unavailable release remote"
else
  unavailable_remote_status=$?
fi
[ "$unavailable_remote_status" -eq 4 ] ||
  fail "unavailable-remote preflight returned an unexpected exit code"
printf '%s\n' "$unavailable_remote_output" | jq -e '
  .verdict == "BLOCKED"
  and any(.findings[]; .code == "REMOTE_TAG_QUERY_FAILED")
' >/dev/null || fail "preflight omitted REMOTE_TAG_QUERY_FAILED"

echo "RELEASE_TOOLING_TEST_OK targets=3 native=$host_target sabotage=line-endings,runbook-refspec,workflow-sha,workflow-final-tag,manifest,rustc-host,archive,binary-format,aggregate,run-receipt,run-rest-identity,formula,selected-run,public-state,public-assets,public-digest,public-tag-binding,remote-tag,tag-absence,preflight"

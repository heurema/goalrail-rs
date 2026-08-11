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

workflow=.github/workflows/release.yml
grep -F 'workflow_dispatch:' "$workflow" >/dev/null ||
  fail "release workflow is not manually dispatched"
if grep -Eq '^[[:space:]]+push:|contents:[[:space:]]*write|gh release|softprops/action-gh-release' "$workflow"; then
  fail "release workflow can publish instead of remaining build-only"
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
  'check-remote-release-tag.sh' \
  'goalrail-${{ inputs.tag }}-*.json' \
  'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1' \
  'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a' \
  'actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c'; do
  grep -F "$marker" "$workflow" >/dev/null ||
    fail "release workflow is missing invariant: $marker"
done

for script in \
  scripts/prepare-release.sh \
  scripts/package-release.sh \
  scripts/check-release.sh \
  scripts/assemble-release.sh \
  scripts/check-release-bundle.sh \
  scripts/check-remote-release-tag.sh \
  scripts/smoke-release-binary.sh \
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
  "$version" "$source_commit" "$output_root/bundle" >/dev/null
scripts/check-release-bundle.sh \
  "$version" "$source_commit" "$output_root/bundle" >/dev/null

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
  "$version" "$source_commit" "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted a mismatched aggregate manifest"
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
  "$version" "$source_commit" "$output_root/bundle" >/dev/null 2>&1; then
  fail "release tooling accepted an explicit formula version"
fi

remote_repo="$fixture_root/remote.git"
remote_work="$fixture_root/remote-work"
git init -q --bare "$remote_repo"
git init -q "$remote_work"
git -C "$remote_work" config user.name fixture
git -C "$remote_work" config user.email fixture@example.invalid
git -C "$remote_work" commit -q --allow-empty -m first
remote_commit=$(git -C "$remote_work" rev-parse HEAD)
git -C "$remote_work" tag -a v1.2.3 -m release
git -C "$remote_work" push -q "$remote_repo" HEAD:refs/heads/main refs/tags/v1.2.3
scripts/check-remote-release-tag.sh v1.2.3 "$remote_commit" "$remote_repo" >/dev/null
git -C "$remote_work" commit -q --allow-empty -m second
git -C "$remote_work" tag -f -a v1.2.3 -m moved >/dev/null
git -C "$remote_work" push -q --force "$remote_repo" refs/tags/v1.2.3
if scripts/check-remote-release-tag.sh \
  v1.2.3 "$remote_commit" "$remote_repo" >/dev/null 2>&1; then
  fail "remote release tag checker accepted a moved tag"
fi

echo "RELEASE_TOOLING_TEST_OK targets=3 native=$host_target sabotage=manifest,rustc-host,archive,binary-format,aggregate,formula,remote-tag"

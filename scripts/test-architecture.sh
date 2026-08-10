#!/bin/sh

set -eu

fail() {
  echo "architecture fitness test failed: $*" >&2
  exit 1
}

source_root=$(git rev-parse --show-toplevel)
subject="$source_root/scripts/check-architecture.rb"
test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-architecture-test.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

baseline="$test_root/baseline"
mkdir -p "$baseline/crates/gr-inspect-codex"
cp -R "$source_root/crates/gr-inspect-codex/src" \
  "$baseline/crates/gr-inspect-codex/"

cat >"$baseline/metadata.json" <<'JSON'
{
  "packages": [
    {
      "name": "gr",
      "dependencies": [
        {"name": "clap", "kind": null, "path": null},
        {"name": "gr-inspect-codex", "kind": null, "path": "/fixture/gr-inspect-codex"}
      ]
    },
    {
      "name": "gr-inspect-codex",
      "dependencies": [
        {"name": "serde", "kind": null, "path": null}
      ]
    },
    {"name": "gr-site", "dependencies": []}
  ]
}
JSON

run_check() {
  fixture_root=$1
  output=$2
  GOALRAIL_ARCHITECTURE_ROOT="$fixture_root" \
    GOALRAIL_ARCHITECTURE_METADATA_FILE="$fixture_root/metadata.json" \
    ruby "$subject" >"$output" 2>&1
}

baseline_output="$test_root/baseline.out"
run_check "$baseline" "$baseline_output" || {
  sed -n '1,100p' "$baseline_output" >&2
  fail "the matching architecture fixture was rejected"
}
for marker in \
  'AD-1: MANUAL' \
  'AD-2: MANUAL' \
  'AD-3: PASS' \
  'AD-4: PASS' \
  'AD-5: PASS' \
  'Architecture fitness v0: PASS (automated scope)'; do
  grep -Fq "$marker" "$baseline_output" ||
    fail "the passing receipt omitted $marker"
done

expect_failure() {
  fixture_root=$1
  expected=$2
  output="$fixture_root/failure.out"
  if run_check "$fixture_root" "$output"; then
    fail "a broken architecture fixture passed: $expected"
  fi
  grep -Fq "$expected" "$output" || {
    sed -n '1,100p' "$output" >&2
    fail "the failure did not identify $expected"
  }
  ad_count=$(grep -Ec '^AD-[1-5]:' "$output" || true)
  test "$ad_count" -eq 5 || {
    sed -n '1,100p' "$output" >&2
    fail "the failed receipt did not report all five ADs"
  }
  grep -Fq 'Architecture fitness v0: FAILED' "$output" ||
    fail "the failed receipt omitted its aggregate result"
}

missing_member="$test_root/missing-member"
cp -R "$baseline" "$missing_member"
ruby -rjson -e '
  path = ARGV.fetch(0)
  data = JSON.parse(File.read(path))
  data.fetch("packages").reject! { |package| package.fetch("name") == "gr-site" }
  File.write(path, JSON.pretty_generate(data))
' "$missing_member/metadata.json"
expect_failure "$missing_member" "AD-3 workspace members"
grep -Fq 'AD-3: FAILED' "$missing_member/failure.out" ||
  fail "a missing member did not fail AD-3"
grep -Fq 'AD-5: FAILED' "$missing_member/failure.out" ||
  fail "a missing gr-site member did not fail AD-5"

unexpected_edge="$test_root/unexpected-edge"
cp -R "$baseline" "$unexpected_edge"
ruby -rjson -e '
  path = ARGV.fetch(0)
  data = JSON.parse(File.read(path))
  site = data.fetch("packages").find { |package| package.fetch("name") == "gr-site" }
  site.fetch("dependencies") << {
    "name" => "gr-inspect-codex",
    "kind" => nil,
    "path" => "/fixture/gr-inspect-codex"
  }
  File.write(path, JSON.pretty_generate(data))
' "$unexpected_edge/metadata.json"
expect_failure "$unexpected_edge" "AD-3 owned dependency edges"
grep -Fq 'AD-5: FAILED' "$unexpected_edge/failure.out" ||
  fail "an owned gr-site dependency did not fail AD-5"

facade_leak="$test_root/facade-leak"
cp -R "$baseline" "$facade_leak"
ruby -e '
  path = ARGV.fetch(0)
  source = File.read(path)
  marker = "#[cfg(test)]"
  abort "test marker missing" unless source.include?(marker)
  File.write(path, source.sub(marker, "pub fn leaked_probe() {}\n\n#{marker}"))
' "$facade_leak/crates/gr-inspect-codex/src/lib.rs"
expect_failure "$facade_leak" "AD-4 public surface"
grep -Fq 'AD-3: PASS' "$facade_leak/failure.out" ||
  fail "a facade failure obscured the passing AD-3 result"
grep -Fq 'AD-4: FAILED' "$facade_leak/failure.out" ||
  fail "a facade leak did not fail AD-4"

signature_drift="$test_root/signature-drift"
cp -R "$baseline" "$signature_drift"
ruby -e '
  path = ARGV.fetch(0)
  source = File.read(path)
  before = "pub fn to_human(&self) -> String {"
  after = "pub fn to_human(&mut self) -> String {"
  abort "signature marker missing" unless source.include?(before)
  File.write(path, source.sub(before, after))
' "$signature_drift/crates/gr-inspect-codex/src/use_case.rs"
expect_failure "$signature_drift" "AD-4 public surface"
grep -Fq 'AD-4: FAILED' "$signature_drift/failure.out" ||
  fail "a public signature change did not fail AD-4"

multiline_signature_drift="$test_root/multiline-signature-drift"
cp -R "$baseline" "$multiline_signature_drift"
ruby -e '
  path = ARGV.fetch(0)
  source = File.read(path)
  before = "pub fn inspect_codex() -> InspectionOutcome {"
  after = <<~RUST.chomp
    pub fn inspect_codex(
        unexpected: bool,
    ) -> InspectionOutcome {
  RUST
  abort "multiline signature marker missing" unless source.include?(before)
  File.write(path, source.sub(before, after))
' "$multiline_signature_drift/crates/gr-inspect-codex/src/use_case.rs"
expect_failure "$multiline_signature_drift" "AD-4 public surface"
grep -Fq 'unexpected: bool' "$multiline_signature_drift/failure.out" ||
  fail "the AD-4 snapshot omitted a multiline signature continuation"

late_impl_leak="$test_root/late-impl-leak"
cp -R "$baseline" "$late_impl_leak"
ruby -e '
  path = ARGV.fetch(0)
  File.open(path, "a") do |file|
    file.write("\nimpl InspectionOutcome {\n")
    file.write("    pub fn leaked_after_tests(&self) {}\n")
    file.write("}\n")
  end
' "$late_impl_leak/crates/gr-inspect-codex/src/use_case.rs"
expect_failure "$late_impl_leak" "AD-4 public surface"
grep -Fq 'AD-4: FAILED' "$late_impl_leak/failure.out" ||
  fail "a public method after the test module did not fail AD-4"

verdict_leak="$test_root/verdict-leak"
cp -R "$baseline" "$verdict_leak"
ruby -e '
  path = ARGV.fetch(0)
  source = File.read(path)
  marker = "    Incomplete,"
  abort "verdict marker missing" unless source.include?(marker)
  File.write(path, source.sub(marker, "#{marker}\n    Unknown,"))
' "$verdict_leak/crates/gr-inspect-codex/src/lib.rs"
expect_failure "$verdict_leak" "AD-4 Verdict variants"

invalid_metadata="$test_root/invalid-metadata"
cp -R "$baseline" "$invalid_metadata"
printf '%s\n' '{' >"$invalid_metadata/metadata.json"
expect_failure "$invalid_metadata" "metadata JSON is invalid"
for marker in \
  'AD-3: NOT_RUN' \
  'AD-4: NOT_RUN' \
  'AD-5: NOT_RUN'; do
  grep -Fq "$marker" "$invalid_metadata/failure.out" ||
    fail "invalid metadata did not report $marker"
done

echo "Architecture fitness tests passed"

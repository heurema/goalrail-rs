#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd "$script_dir/.." && pwd)"
output_dir="${1:-$project_root/dist/site}"
public_dir="$project_root/crates/gr-site/public"
wasm_target="wasm32-unknown-unknown"

if ! rustup target list --installed | grep -qx "$wasm_target"; then
  echo "Missing Rust target: $wasm_target" >&2
  echo "Install it explicitly with: rustup target add $wasm_target" >&2
  exit 2
fi

cargo build \
  --manifest-path "$project_root/Cargo.toml" \
  --package gr-site \
  --target "$wasm_target" \
  --release

install -d "$output_dir"
install -m 0644 "$public_dir/index.html" "$output_dir/index.html"
install -m 0644 "$public_dir/styles.css" "$output_dir/styles.css"
install -m 0644 "$public_dir/site.js" "$output_dir/site.js"
install -m 0644 "$public_dir/status.json" "$output_dir/status.json"
install -m 0644 "$public_dir/install.md" "$output_dir/install.md"
install -m 0644 "$public_dir/llms.txt" "$output_dir/llms.txt"
install -m 0644 \
  "$project_root/target/$wasm_target/release/gr_site.wasm" \
  "$output_dir/gr_site.wasm"

echo "Built Goalrail site at $output_dir"

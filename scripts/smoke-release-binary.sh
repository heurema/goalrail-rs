#!/bin/sh

set -eu

usage() {
  echo "Usage: $0 <version> <binary>" >&2
  exit 64
}

fail() {
  echo "smoke-release-binary: $1" >&2
  exit 1
}

[ "$#" -eq 2 ] || usage
version=$1
binary=$2

if ! printf '%s\n' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  fail "version must use stable SemVer form: $version"
fi

command -v jq >/dev/null 2>&1 || fail "jq is unavailable"
command -v rustc >/dev/null 2>&1 || fail "rustc is unavailable"
[ -f "$binary" ] || fail "release binary is missing: $binary"
case "$binary" in
  /*) ;;
  *)
    binary_dir=$(CDPATH='' cd -- "$(dirname -- "$binary")" && pwd)
    binary="$binary_dir/$(basename -- "$binary")"
    ;;
esac

test_root=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-native-smoke.XXXXXX")
cleanup() {
  rm -rf "$test_root"
}
trap cleanup EXIT HUP INT TERM

fake_bin="$test_root/bin"
codex_home="$test_root/codex-home"
project="$test_root/project"
mkdir -p "$fake_bin" "$codex_home/skills/example" "$project/.git"
printf '%s\n' '# Example' >"$codex_home/skills/example/SKILL.md"

cat >"$test_root/fake-codex.rs" <<'EOF'
use std::{fmt::Write as _, io::{self, BufRead, Write}};

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", character as u32).unwrap();
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--version"] => println!("codex-cli release-smoke"),
        ["doctor", "--json"] => println!(
            "{{\"schemaVersion\":1,\"overallStatus\":\"ok\",\"codexVersion\":\"release-smoke\",\"checks\":{{\"fixture\":{{\"id\":\"fixture\",\"status\":\"ok\",\"summary\":\"fixture ok\"}}}}}}"
        ),
        ["features", "list"] => println!("fixture stable true"),
        ["plugin", "list", "--json"] => {
            println!("{{\"installed\":[],\"available\":[]}}")
        }
        ["plugin", "marketplace", "list", "--json"] => {
            println!("{{\"marketplaces\":[]}}")
        }
        ["mcp", "list", "--json"] => println!("[]"),
        ["app-server"] | ["app-server", "--stdio"] => serve_app_server(),
        _ => std::process::exit(9),
    }
}

fn serve_app_server() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut stdout = io::stdout();

    if lines.next().transpose().unwrap().is_none() {
        std::process::exit(8);
    }
    let codex_home = std::env::var("CODEX_HOME").unwrap();
    writeln!(
        stdout,
        "{{\"id\":1,\"result\":{{\"codexHome\":{}}}}}",
        json_string(&codex_home)
    )
    .unwrap();
    stdout.flush().unwrap();

    if lines.next().transpose().unwrap().is_none() {
        std::process::exit(8);
    }
    let cwd = std::env::current_dir().unwrap();
    let skill = std::path::Path::new(&codex_home).join("skills/example/SKILL.md");
    writeln!(
        stdout,
        "{{\"id\":2,\"result\":{{\"data\":[{{\"cwd\":{},\"skills\":[{{\"name\":\"example\",\"path\":{},\"scope\":\"user\",\"enabled\":true}}],\"errors\":[]}}]}}}}",
        json_string(&cwd.to_string_lossy()),
        json_string(&skill.to_string_lossy())
    )
    .unwrap();
    stdout.flush().unwrap();
}
EOF

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    command -v cygpath >/dev/null 2>&1 || fail "cygpath is unavailable"
    fake_codex="$fake_bin/codex.exe"
    codex_home_env=$(cygpath -w "$codex_home")
    ;;
  *)
    fake_codex="$fake_bin/codex"
    codex_home_env=$codex_home
    ;;
esac
rustc "$test_root/fake-codex.rs" -o "$fake_codex"

stdout_file="$test_root/stdout.json"
stderr_file="$test_root/stderr.txt"
if (cd "$project" && PATH="$fake_bin" CODEX_HOME="$codex_home_env" \
  "$binary" inspect codex --json) >"$stdout_file" 2>"$stderr_file"; then
  status=0
else
  status=$?
fi

if [ "$status" -ne 0 ]; then
  sed -n '1,120p' "$stdout_file" >&2
  sed -n '1,80p' "$stderr_file" >&2
  fail "native inspection smoke exited with $status"
fi

if ! jq -e '
  .schemaVersion == 1
  and .verdict == "BASELINE_OK"
  and .skills.active == 1
  and .plugins.installed == 0
  and .mcp.configured == 0
' "$stdout_file" >/dev/null; then
  sed -n '1,160p' "$stdout_file" >&2
  fail "native inspection smoke returned unexpected evidence"
fi

printf 'RELEASE_BINARY_SMOKE_OK version=%s binary=%s verdict=BASELINE_OK\n' \
  "$version" "$binary"

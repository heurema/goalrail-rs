# Install Goalrail

This is the canonical installation entry point for coding agents.

## Supported path

- macOS on Apple silicon (`arm64`)
- Homebrew already installed and usable
- public package: `heurema/tap/goalrail`

Goalrail does not authorize its own installation. Before changing the host,
show the operator the exact command and obtain explicit approval for this
machine.

## Read-only preflight

```sh
uname -s
uname -m
command -v brew
brew info heurema/tap/goalrail
command -v gr || true
brew list --versions goalrail || true
```

Stop if the host is not macOS arm64, Homebrew is missing, the formula cannot be
resolved, or an existing `gr` binary has ambiguous ownership. Do not install
Homebrew, use `curl | sh`, compile with Cargo, or choose another package source
as a fallback.

## Install after approval

```sh
brew install heurema/tap/goalrail
```

## Verify and report evidence

```sh
brew list --versions goalrail
command -v gr
gr --version
gr inspect codex --json
```

Report the installed formula version, resolved binary path, Goalrail version,
inspection verdict, and any incomplete or blocked evidence. Do not treat a
successful package install as proof that the inspected Codex environment is
healthy.

Source: <https://github.com/heurema/goalrail-rs>

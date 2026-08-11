# Goalrail CLI package lifecycle

These instructions manage the native `gr` binary and Homebrew package only.
They never install, update, or remove the Goalrail Codex plugin or marketplace.

The canonical source is <https://goalrail.dev/install.md>. This bundled copy
keeps the Codex skill usable when the site is unavailable. It supports only
macOS on Apple silicon with an existing Homebrew installation.

## Read-only preflight

```sh
uname -s
uname -m
command -v brew
brew info heurema/tap/goalrail
command -v gr || true
brew list --versions goalrail || true
```

Stop when the host is not macOS arm64, Homebrew is missing, the formula cannot
be resolved, or an existing `gr` has ambiguous ownership. Do not install
Homebrew, use `curl | sh`, compile from source, or choose another package source
as a fallback.

## State-changing actions

Show exactly one applicable command, name the target as the native Goalrail
CLI package, and obtain approval for that action and machine before running it:

```sh
brew install heurema/tap/goalrail
brew upgrade heurema/tap/goalrail
brew uninstall heurema/tap/goalrail
```

Do not bundle approval for install, upgrade, or uninstall with the user's
inspection request.

## Verification after install or upgrade

```sh
brew list --versions goalrail
command -v gr
gr --version
gr inspect codex --json
```

Report the formula version, binary path, Goalrail version, inspection verdict,
and incomplete or blocked evidence. Package installation alone does not prove
that the inspected Codex environment is healthy.

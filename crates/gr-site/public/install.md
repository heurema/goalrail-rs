# Install and update Goalrail

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
HOMEBREW_NO_AUTO_UPDATE=1 brew info heurema/tap/goalrail
command -v gr || true
HOMEBREW_NO_AUTO_UPDATE=1 brew list --versions goalrail || true
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
HOMEBREW_NO_AUTO_UPDATE=1 brew list --versions goalrail
command -v gr
gr --version
gr inspect codex --json
```

Report the installed formula version, resolved binary path, Goalrail version,
inspection verdict, and any incomplete or blocked evidence. Do not treat a
successful package install as proof that the inspected Codex environment is
healthy.

Source: <https://github.com/heurema/goalrail-rs>

## Learn about updates

Goalrail does not check for updates in the background. Ask an agent with the
Goalrail skill to "check Goalrail updates". It checks the native CLI package
and Codex plugin as two independent channels and does not issue an update
command without explicit approval. Codex itself may reconcile configured Git
marketplaces and plugin caches when a task starts, before the agent runs. The
agent must disclose that host behavior instead of calling a new task a
read-only check.

For human notifications, select `Watch` -> `Custom` -> `Releases` on
<https://github.com/heurema/goalrail-rs>. Watching is optional and is never
enabled by Goalrail or its agent.

For an agent check, the latest non-draft GitHub Release is the common baseline.
Homebrew and the Codex marketplace are compared with it separately. If a
published release exists but a managed channel has not advanced yet, an
installation already at the channel version is `CHANNEL_LAG` with
`WAIT_FOR_CHANNEL`. If the installation is even older, the agent reports the
available intermediate update with `channelLag: true` and explains that waiting
avoids a likely second update. Neither case becomes `NO_CHANGE` or an unmanaged
overwrite.

## Check the native CLI package

The agent reads the installed package and the Homebrew tap snapshot:

```sh
HOMEBREW_NO_AUTO_UPDATE=1 brew list --versions goalrail || true
HOMEBREW_NO_AUTO_UPDATE=1 brew tap-info --json heurema/tap
git -C <tap-root> status --porcelain --untracked-files=no
git -C <tap-root> symbolic-ref --short HEAD
git -C <tap-root> rev-parse HEAD
git ls-remote --exit-code https://github.com/heurema/homebrew-tap.git refs/heads/main
```

`brew info` does not refresh a third-party tap. The agent may return
`NO_CHANGE` only when the local tap commit matches the remote `main` commit,
the fresh formula version equals both the installed version and the latest
non-draft GitHub Release, and Homebrew reports no outdated Goalrail formula. A
different commit is `STALE_METADATA`, not proof that an update exists or that
nothing changed.

After proving tap freshness, the installed Goalrail skill runs its bundled
`scripts/homebrew-update-state.sh` helper. That helper is the single
Goalrail-owned boundary for interpreting structured `brew info` and
`brew outdated` output, including Homebrew's update-present exit status `1`.
The agent must preserve a blocked helper finding instead of rebuilding the
probe with ad hoc shell commands.

Refreshing metadata is a separate change. After approval, the agent may run
`brew update` once, explain that it refreshes Homebrew and tap metadata beyond
Goalrail, and repeat the read-only check. If a newer formula is then available,
the agent asks separately before running:

```sh
HOMEBREW_NO_AUTO_UPDATE=1 brew upgrade heurema/tap/goalrail
```

For a direct GitHub Release installation, including the current Linux and
Windows bundles, the agent can compare `gr --version` with the live public
GitHub Release and require the exact platform asset, checksum, and manifest.
Those platforms do not yet have a Goalrail-owned package manager, installation
receipt, or updater. A newer version is reported as `UPDATE_AVAILABLE` with
`MANUAL_REINSTALL_REQUIRED`; the agent must not overwrite an ambiguously owned
binary.

## Check the Codex plugin

The plugin marketplace is separate from Homebrew. Within the current task, the
agent records the existing Codex config, marketplace snapshot, optional
receipt, and plugin cache, then uses the structured Codex lists:

```sh
codex plugin marketplace list --json
codex plugin list --marketplace goalrail --json
```

An isolated stale-snapshot canary found that the current list commands changed
neither the snapshot nor config. The agent still compares before/after evidence
so future Codex behavior drift is visible. It does not infer an installed
version from an orphan cache. It compares the configured `goalrail` snapshot
commit with `heurema/goalrail-rs` `main`, rejects tracked local changes, and
verifies source `heurema/goalrail-rs`, ref `main`, and any available receipt or
config revision. A stale or unverified snapshot cannot produce `NO_CHANGE`.

After separate approval, the manual path refreshes only the Goalrail
marketplace:

```sh
codex plugin marketplace upgrade goalrail --json
```

The agent then validates the advertised immutable Goalrail tag. Applying an
available plugin update is another separately approved action:

```sh
codex plugin add goalrail@goalrail --json
```

The task that performed the update may still have the previous skill
instructions loaded. Before asking the user to open a new task or restart
Codex, the agent explains that this reload may itself trigger host
reconciliation and is not a read-only verification step.

# Goalrail update discovery

Use this flow only when the user explicitly asks to check for or apply Goalrail
updates. Do not run it during ordinary inspection.

Goalrail has two independently managed channels:

- `CLI distribution`: the native `gr` binary, managed by Homebrew when that
  ownership is proved and otherwise observed through the public GitHub Release
  channel without claiming an updater;
- `Codex plugin`: the Goalrail skill owned by the Codex host and marketplace.

Checking one channel says nothing about the other. Check both for a general
"Goalrail updates" request, or only the named channel when the user is
specific. The agent must not issue a metadata-refresh or update command without
separate approval for that exact action.

There is one important host boundary. In a live canary, starting a new Codex
task reconciled a configured Git marketplace and replaced the cached Goalrail
plugin before the agent's first command. This is Codex host behavior, not a
Goalrail command, and it is not documented as a stable lifecycle contract.
Never promise that opening or restarting a Codex task is a read-only update
check. Disclose this boundary when the plugin channel is in scope.

## Result contract

Report one compact row per channel with `installed`, `available`, `freshness`,
`verdict`, and `nextAction`. Add `manager` to the CLI row. For the plugin row, add
`hostLifecycle: HOST_MANAGED` and `startupReconciliation`. Set the latter to
`OBSERVED` only with pre-start before/after evidence; otherwise use
`UNVERIFIED`, even when current state is `NO_CHANGE`.

Report one common `latestRelease` from the public release baseline below.
`available` means the version exposed by that channel's manager or catalog; it
is not automatically the latest public Goalrail release.

Freshness is one of:

- `FRESH`: a live source record was verified, and any local metadata snapshot
  equals the observed remote channel head;
- `STALE_METADATA`: the commits differ; do not use the cached available version
  or return `NO_CHANGE`;
- `UNVERIFIED`: source identity, local snapshot, or remote head could not be
  proved.

Channel verdicts are `NO_CHANGE`, `UPDATE_AVAILABLE`, `CHANNEL_LAG`,
`NOT_INSTALLED`, `STALE_METADATA`, `HOST_RECONCILED`, or `BLOCKED`. Overall
`NO_CHANGE` requires a fresh public release baseline and every installed channel
in scope to have verdict `NO_CHANGE`. That verdict requires its proved installed
version, fresh manager or catalog version, and `latestRelease` to be equal.
Never compare semantic versions lexicographically.

Require `git` for Git-backed channels and an already available structured HTTP
client for the common release baseline; do not install an enabling tool. A
failed remote query, malformed structured file, duplicate identity, dirty
tracked snapshot, ambiguous installed cache, or missing required field makes
freshness `UNVERIFIED` and the channel `BLOCKED`.

## Public release baseline

Query this live public record once for the whole request, without downloading
an asset or changing an installation:

```sh
curl --fail --silent --show-error --location \
  https://api.github.com/repos/heurema/goalrail-rs/releases/latest
```

Use an already available structured HTTP client; do not install one. Parse JSON
instead of scraping HTML. Require both `draft` and `prerelease` to be `false`, a
semantic `v<version>` tag, and a `release.json` asset. Record that version as
`latestRelease`. A failed request or malformed record makes the baseline
`UNVERIFIED`; no channel may return `NO_CHANGE`.

Compare each fresh manager or catalog version with `latestRelease`:

- equal: continue with its installed-version comparison;
- lower and equal to the installed version: return `CHANNEL_LAG` with
  `nextAction: WAIT_FOR_CHANNEL`;
- lower but newer than the installed version: return `UPDATE_AVAILABLE` for the
  manager or catalog version with `channelLag: true`. Disclose that it is not
  the latest public release and that waiting avoids a likely second update,
  but require the normal channel-specific approval if the user chooses the
  intermediate update;
- lower than the installed version: return `BLOCKED`; never downgrade;
- higher: return `BLOCKED` because a manager or catalog advertises a version
  newer than the latest public release.

## CLI distribution channel

### Homebrew-managed path

Read `install.md` and complete its ownership preflight first. Then inspect the
installed package and structured tap metadata:

```sh
HOMEBREW_NO_AUTO_UPDATE=1 brew list --versions goalrail || true
HOMEBREW_NO_AUTO_UPDATE=1 brew tap-info --json heurema/tap
```

Require one installed `heurema/tap` entry whose remote identifies
`heurema/homebrew-tap`. Use its reported `path` as `<tap-root>`, then obtain the
branch and commit from Git and require no tracked local change:

```sh
git -C <tap-root> status --porcelain --untracked-files=no
git -C <tap-root> symbolic-ref --short HEAD
git -C <tap-root> rev-parse HEAD
```

The status output must be empty, the symbolic branch must be `main`, and the
resolved commit must be a full SHA. If optional `branch` or `HEAD` fields are
present in Homebrew JSON, require them to match Git; their absence is not a
failure. Query the public channel without changing local metadata:

```sh
git ls-remote --exit-code https://github.com/heurema/homebrew-tap.git refs/heads/main
```

If the local and remote commits differ, report `STALE_METADATA`, leave
`available` unknown, and stop this channel. `brew info` reads the local tap; it
does not refresh it and therefore cannot justify `NO_CHANGE` while the commits
differ.

Only when the commits match, resolve the directory containing the loaded
Goalrail `SKILL.md` and execute its bundled diagnostic:

```sh
<goalrail-skill-root>/scripts/homebrew-update-state.sh
```

Do not reconstruct the two Homebrew probes in the agent. This script is the
only Goalrail-owned interpreter of `brew info` and `brew outdated`. It disables
auto-update, validates their structured output together, and normalizes
Homebrew's successful outdated exit status `1`. A nonzero helper exit is
`BLOCKED`; preserve its finding instead of retrying or reparsing the raw output.

Apply the public-baseline mapping above to the helper's installed and available
versions before deciding the channel verdict. When the current formula equals
`latestRelease`, helper `NO_CHANGE` means channel `NO_CHANGE`; helper
`UPDATE_AVAILABLE` means `UPDATE_AVAILABLE` and
`nextAction: REQUEST_HOMEBREW_UPGRADE_APPROVAL`. When the fresh formula trails
`latestRelease`, report `CHANNEL_LAG` if it is already installed, or
`UPDATE_AVAILABLE` with `channelLag: true` if it is newer than the installation.
Helper `NOT_INSTALLED` maps to channel `NOT_INSTALLED`. Any contradictory
structured output is `BLOCKED`. Set `manager: HOMEBREW`.

When metadata is stale, explain that `brew update` refreshes Homebrew and tap
metadata, not just Goalrail. Obtain approval for that refresh before running it
once:

```sh
brew update
```

Repeat discovery from the beginning. If an update is then available, obtain
separate approval for the exact package mutation from `install.md`. Do not treat
approval to check or refresh as approval to upgrade.

### Direct release observation

If the Homebrew ownership preflight does not apply, do not pretend the CLI is
absent and do not choose an installer. Read the resolved binary and version,
then identify one released target:

```sh
command -v gr || true
gr --version
uname -s
uname -m
```

If no unambiguous `gr` executable is present, report `NOT_INSTALLED` and stop
this channel.

On Windows use the shell's native executable lookup and runtime OS/architecture
facts. The public direct-release targets are exactly:

- macOS arm64: `aarch64-apple-darwin`;
- Linux x86_64: `x86_64-unknown-linux-gnu`;
- Windows x86_64: `x86_64-pc-windows-msvc`.

Other OS/architecture combinations are `BLOCKED`. The live public release
baseline must also contain the exact target's `.tar.gz`, `.tar.gz.sha256`, and
`.json` assets. A valid live response is `FRESH`. Compare `latestRelease` with
`gr --version` semantically. An equal version is `NO_CHANGE`; a newer public
version is `UPDATE_AVAILABLE` with
`nextAction: MANUAL_REINSTALL_REQUIRED`. An installed version newer than the
public release, a missing target asset, or ambiguous binary path is `BLOCKED`.

Linux and Windows currently have no Goalrail-owned package manager, installer,
installation receipt, or updater. Do not download an archive, overwrite the
resolved executable, or invent an update command. Set `manager` to
`DIRECT_RELEASE_UNMANAGED` and link the exact GitHub Release. A managed
cross-platform update path is a separate future decision.

## Codex plugin channel

Read `plugin-lifecycle.md`. Starting the task may already have reconciled the
plugin, so disclose that boundary before reporting current state. Once the
agent is running, the current Codex list commands are observation surfaces:

```sh
codex plugin marketplace list --json
codex plugin list --marketplace goalrail --json
```

An isolated canary on Codex CLI `0.147.0-alpha.6.5` ran each command against a
deliberately stale Git snapshot; neither changed its commit nor `config.toml`.
Treat that as version-scoped evidence, not a permanent upstream guarantee.
Before running the commands, record a normalized digest of only the Goalrail
marketplace and plugin config tables, plus the Goalrail snapshot commit,
optional receipt, and Goalrail cache inventory. Compare them afterward. Any
unexpected change is `HOST_RECONCILED`; report the before/after evidence and
stop instead of silently retrying.

A later live update on Codex CLI `0.147.0` observed
`codex plugin marketplace upgrade goalrail --json` refresh the snapshot and
replace the installed Goalrail cache during the same command. Treat marketplace
upgrade as a potentially combined catalog-and-plugin reconciliation, not as a
metadata-only operation. This is version-scoped evidence rather than a stable
upstream contract.

Resolve `<codex-home>` from the active Codex profile without creating or
changing it. Read only `[marketplaces.goalrail]` and
`[plugins."goalrail@goalrail"]` from `<codex-home>/config.toml` with a TOML-aware
reader; do not print unrelated configuration. Require exactly one marketplace
named `goalrail` from the JSON list, with the same existing root and Git source
`https://github.com/heurema/goalrail-rs.git`. The config entry must record the
same Git source and ref `main`.

Require the marketplace root to have no tracked local change and resolve its
snapshot commit:

```sh
git -C <marketplace-root> status --porcelain --untracked-files=no
git -C <marketplace-root> rev-parse HEAD
```

Codex CLI registration does not always create a snapshot receipt or config
`last_revision`. Their absence alone is not a failure. If `last_revision` is
present, it must equal the snapshot commit. If
`<marketplace-root>/.codex-marketplace-install.json` is present, it must record
Git source `https://github.com/heurema/goalrail-rs.git`, `ref_name` `main`, and a
`revision` equal to that commit. The receipt may be untracked; do not mistake
expected Codex metadata for a modified catalog. Query the supported remote
catalog channel without changing the snapshot:

```sh
git ls-remote --exit-code https://github.com/heurema/goalrail-rs.git refs/heads/main
```

If the commits differ, report `STALE_METADATA`, leave `available` unknown, and
stop this channel. Do not call the cached marketplace's plugin version latest.
Before proposing reconciliation, resolve the directory containing the loaded
Goalrail `SKILL.md` and run its bundled diagnostic against the exact observed
remote commit without changing the local snapshot:

```sh
<goalrail-skill-root>/scripts/plugin-update-target.sh \
  <remote-commit> <latestRelease>
```

The diagnostic reads the exact remote marketplace, plugin manifest, required
skill tree, Git blob inventory, and peeled immutable tag. It requires the tag
payload commit to equal `<remote-commit>` and returns `TARGET_VERIFIED` with a
`remoteCandidate` and payload inventory. Preserve any `BLOCKED` finding instead
of reconstructing or weakening the probe. Report `remoteCandidate`, not
`available`, while the configured snapshot remains stale.

Only when the commits match, inspect
`<marketplace-root>/.agents/plugins/marketplace.json` and validate the exact
repository, `./plugins/goalrail` path, immutable `v<version>` ref, and resolved
tag as required by `plugin-lifecycle.md`. Peel an annotated tag to its commit;
do not report the tag-object SHA as the payload commit.

Use only the exact `goalrail@goalrail` entry from the plugin JSON as installed
state. No entry is `NOT_INSTALLED`; duplicate entries, a missing version,
unexpected source, or invalid path is `BLOCKED`. For an installed entry, require
source kind `git-subdir`, URL
`https://github.com/heurema/goalrail-rs.git`, ref `v<installed-version>`, and
path spelled exactly `plugins/goalrail` or `./plugins/goalrail`. These two path
spellings are equivalent; reject every other path, including an absolute path,
an empty component, any other `.` or `..` component, and backslash-separated
variants.
Require its exact versioned cache manifest to exist at
`<codex-home>/plugins/cache/goalrail/goalrail/<version>/.codex-plugin/plugin.json`
and to contain the same name and semantic version. Ignore unrelated orphan
cache versions; never choose the highest cache directory as installed state.
Compare the proved installed version with the validated advertised version.
Apply the public-baseline mapping first. When the advertised version equals
`latestRelease`, a lower installed version is `UPDATE_AVAILABLE` with
`nextAction: REQUEST_PLUGIN_ADD_APPROVAL`, an equal version is `NO_CHANGE`, and
a higher installed version is `BLOCKED`. When the catalog trails
`latestRelease`, report `CHANNEL_LAG` if its version is already installed, or
`UPDATE_AVAILABLE` with `channelLag: true` if it is newer than the installation.
Never let an exposed catalog version satisfy overall `NO_CHANGE` while the
proved installed version is lower.

For a deliberate manual update, treat marketplace upgrade as one
approval-gated host reconciliation action that may change both the catalog
snapshot and the installed Goalrail plugin. Before asking, present the exact
remote commit, validated `remoteCandidate` version, immutable payload tag and
commit, and the installed version. The approval must name both possible local
changes. Record direct config, snapshot, receipt, and cache evidence immediately
before running. Immediately re-read remote `main` with the documented
`git ls-remote` command and require it to equal the approved commit. A mismatch
is `STALE_METADATA`; do not run the mutation. This reduces but cannot eliminate
the moving-ref race because Codex does not expose a commit-bound marketplace
upgrade. Post-command exact snapshot and cache verification remains mandatory.
Then run:

```sh
codex plugin marketplace upgrade goalrail --json
```

Read config, snapshot, receipt, and cache directly after the command and before
calling either list command. When the cache version changed, rerun the same
diagnostic with the exact installed cache root as its third argument. Only
`TARGET_VERIFIED` with `cacheVerified: true` proves that every installed file
matches the approved Git blob inventory. If the snapshot moved to the approved
commit and this proof passes, report `UPDATED`; do not run `plugin add`. If the
snapshot moved exactly but the installed plugin is unchanged, report that
narrower result, repeat structured discovery, and only then request separate
approval for:

```sh
codex plugin add goalrail@goalrail --json
```

An unexpected version, source, path, commit, cache change, or pre-command drift
is `HOST_RECONCILED` or `BLOCKED`; stop the current task instead of retrying an
update command. The current task may still have the previous skill instructions
loaded after either update path. Before asking for a reload task, construct an
external bootstrap prompt from the immediate post-command proof. It must include
`expectedVersion` and the exact `expectedSkillManifest` path under the verified
versioned cache. The prompt must tell the new task, before reading or following
any Goalrail skill instructions, to compare its task-registered Goalrail skill
path with `expectedSkillManifest` and verify that the enclosing plugin manifest
has `expectedVersion`. It must not choose a path from cache inventory or execute
the registered skill first.

Use a new task with that external bootstrap prompt as the normal reload action.
If the registration is missing, the path differs, or the manifest version is
not `expectedVersion`, return `BLOCKED` for that verification task and report
stale host registration; do not claim that the update failed or continue into
the loaded lifecycle instructions. Offer at most one additional new task with
the same bootstrap values before a client restart. Treat client restart only as
a fallback after the repeated registration failure; never say it is required
after one stale task. Explain before each reload action that every new task and
any restart may also trigger Codex host reconciliation, so none is a read-only
verification step.

## Notifications

The Goalrail skill and CLI perform no background update check. The Codex host
may independently reconcile configured Git marketplaces at task startup; do
not attribute that behavior to Goalrail or claim it as a stable notification
mechanism. For human notifications, offer `Watch` -> `Custom` -> `Releases` on
<https://github.com/heurema/goalrail-rs>; do not enable it on the user's behalf.

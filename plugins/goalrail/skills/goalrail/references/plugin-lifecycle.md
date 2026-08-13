# Goalrail Codex plugin lifecycle

These instructions manage the Codex plugin and marketplace only. They never
install, upgrade, or uninstall the native `gr` CLI or its Homebrew package.

## Observation preflight

```sh
codex plugin --help
codex plugin marketplace list --json
codex plugin list --marketplace goalrail --json
```

Record the active Codex profile's existing config, Git snapshot, optional
marketplace receipt, and plugin cache before and after the JSON list commands,
using the pre/post evidence defined in the Codex plugin channel of
`update-discovery.md`. Confirm the exact marketplace source and whether
`goalrail@goalrail` is installed before proposing a change. For update intent,
follow the full update-discovery flow; install, registration, and removal
preflight does not apply its public-release comparison. The repository
marketplace file alone does not register the marketplace or install the plugin
in Codex.

Do not describe starting a task or restarting Codex as read-only lifecycle
operations. A live canary observed the Codex host reconcile the Goalrail
marketplace and plugin before a newly started agent issued its first command.
An isolated stale-snapshot canary found that the current JSON list commands did
not change the snapshot or config, so they remain observation surfaces with
pre/post evidence. Current public Codex documentation does not define either
behavior as a permanent lifecycle contract.

## Activate the remote trial

Activation has two separate state-changing steps. Resolve the Git repository
and marketplace channel, then obtain approval for only the next step before
running it. The supported catalog channel is `main`; plugin payloads are pinned
by the marketplace entry to the immutable shared Goalrail `v<version>` tag.

1. Register the Git repository as the `goalrail` marketplace:

   ```sh
   codex plugin marketplace add heurema/goalrail-rs --ref main --json
   ```

   Verify the command result, config, and snapshot, plus the receipt when Codex
   created one, then stop. Registration does not authorize plugin installation.

2. After separate approval, install the plugin:

   ```sh
   codex plugin add goalrail@goalrail --json
   ```

   Verify with the command result, direct cache evidence, and the JSON output of
   `codex plugin list`. Before asking the user to restart Codex, explain that a
   reload may also reconcile configured Git marketplaces and plugins.

The marketplace entry uses a remote `git-subdir` source and must target an
immutable plugin tag, including during the first remote canary. Do not replace
it with a local checkout path or moving payload ref.

## Refresh or removal

The current Codex CLI has no `codex plugin update` command. Never substitute a
Homebrew upgrade. First record the installed version and configured marketplace
source from the direct evidence defined in `update-discovery.md`.

For update discovery, follow `update-discovery.md`. Compare the configured
marketplace snapshot commit with the observed remote `main` commit before
reading its advertised plugin version. A changed commit is `STALE_METADATA`,
not evidence for either `NO_CHANGE` or an available version.

The host may reconcile at task startup. A live Codex CLI `0.147.0` update also
showed that marketplace upgrade can refresh the catalog and replace an enabled
installed Goalrail plugin during the same command. Treat the manual refresh as
a potentially combined state-changing action rather than promising a
metadata-only step.

Before asking for authority, use the exact remote-commit procedure in
`update-discovery.md` to validate the remote catalog entry, immutable tag,
payload commit, previous installed version, and target version. Obtain approval
for one command and explicitly state that it may change both the Goalrail
marketplace snapshot and installed plugin. Immediately before the command,
re-read remote `main` and require it to equal the approved commit. Stop on a
mismatch; the command cannot atomically bind its moving ref to that commit:

```sh
codex plugin marketplace upgrade goalrail --json
```

Read the marketplace root from the command result and inspect direct config,
receipt, snapshot, and cache evidence before calling a list command. Then
inspect `.agents/plugins/marketplace.json`. The `goalrail` entry must advertise
exactly:

- URL `https://github.com/heurema/goalrail-rs.git`;
- path `./plugins/goalrail`;
- ref `v<version>`, matching both the plugin and native CLI release version.

Resolve that exact tag with `git ls-remote`, including the peeled `^{}` ref for
an annotated tag. Never substitute an annotated tag-object SHA for its payload
commit. Approval applies only to the presented evidence. A moving ref,
unresolved or ambiguous tag, version mismatch, unexpected source/path, or
evidence change before the mutation is `BLOCKED`.

If the installed cache changed, run `scripts/plugin-update-target.sh` again with
the exact installed cache root and require `cacheVerified: true`. Then report
`UPDATED` and do not run `plugin add`. If only the snapshot changed and the
installed plugin remained byte-for-byte unchanged, repeat structured discovery
and ask separately before running:

```sh
codex plugin add goalrail@goalrail --json
```

The current `codex plugin add` replaces an already installed plugin with the
version exposed by the refreshed marketplace; it does not require removing the
working installation first. Compare the readback with the recorded version:

- unchanged version and installed path: `NO_CHANGE`;
- changed version with an enabled `goalrail@goalrail`: `UPDATED`;
- command failure, absent plugin, wrong marketplace, or unverifiable version:
  `BLOCKED` without claiming an update.

If the configured marketplace itself is pinned to another immutable ref,
return `BLOCKED` instead of removing it: the current CLI cannot retarget it
atomically. The supported update path keeps the marketplace catalog on `main`
and advances only the entry's immutable remote plugin tag.

Removing the installed plugin and removing the marketplace are distinct
changes. Request exact approval for only one at a time:

```sh
codex plugin remove goalrail@goalrail --json
codex plugin marketplace remove goalrail --json
```

After each approved change, read back the corresponding JSON list. Never
attribute a plugin change to the agent merely because a later readback shows a
new version. Compare pre-command direct evidence with the command result,
immediate direct readback, and later structured readback. If Codex changed the
plugin outside the approved target or command, report `HOST_RECONCILED` and the
observed evidence instead.

After a verified plugin install or replacement, explicitly tell the user that
the current task may still be running the previous skill instructions. Use a
new task as the normal reload action. If it reports a missing or
previous-version Goalrail skill path, return `BLOCKED` for that verification
task and report stale host registration; do not claim that the update failed.
Offer at most one additional new task before treating client restart as a
fallback. Never say that restart is required after one stale task. Warn that
each new task and any restart may also reconcile configured Git marketplaces
and plugins.

## Native CLI boundary

The plugin may detect a missing `gr` and offer the separate Homebrew protocol
from `install.md`. The plugin installation itself never installs the binary,
and Homebrew installation never registers or installs this Codex plugin. Do
not couple their lifecycle mutations or reuse approval between them.

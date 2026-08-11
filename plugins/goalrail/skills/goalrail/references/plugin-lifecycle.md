# Goalrail Codex plugin lifecycle

These instructions manage the Codex plugin and marketplace only. They never
install, upgrade, or uninstall the native `gr` CLI or its Homebrew package.

## Read-only preflight

```sh
codex plugin --help
codex plugin marketplace list --json
codex plugin list --json
```

Confirm the exact marketplace source and whether `goalrail@goalrail` is already
installed before proposing a change. The repository marketplace file alone
does not register the marketplace or install the plugin in Codex.

## Activate the remote trial

Activation has two separate state-changing steps. Resolve the Git repository
and marketplace channel, then obtain approval for only the next step before
running it. The supported catalog channel is `main`; plugin payloads are pinned
by the marketplace entry to the immutable shared Goalrail `v<version>` tag.

1. Register the Git repository as the `goalrail` marketplace:

   ```sh
   codex plugin marketplace add heurema/goalrail-rs --ref main --json
   ```

   Verify with `codex plugin marketplace list --json` and stop. Registration
   does not authorize plugin installation.

2. After separate approval, install the plugin:

   ```sh
   codex plugin add goalrail@goalrail --json
   ```

   Verify with `codex plugin list --json`. Restart Codex if the current client
   does not load a newly installed skill dynamically.

The marketplace entry uses a remote `git-subdir` source and must target an
immutable plugin tag, including during the first remote canary. Do not replace
it with a local checkout path or moving payload ref.

## Refresh or removal

The current Codex CLI has no `codex plugin update` command. Never substitute a
Homebrew upgrade. First record the installed version from
`codex plugin list --json` and inspect the configured marketplace source.

Refreshing the catalog and applying the plugin are two separate state-changing
steps. Obtain approval for only the next step:

```sh
codex plugin marketplace upgrade goalrail --json
codex plugin add goalrail@goalrail --json
```

After the marketplace refresh and before asking for `plugin add` authority,
read the marketplace root from `codex plugin marketplace list --json`, then
inspect its `.agents/plugins/marketplace.json`. The `goalrail` entry must
advertise exactly:

- URL `https://github.com/heurema/goalrail-rs.git`;
- path `./plugins/goalrail`;
- ref `v<version>`, matching both the plugin and native CLI release version.

Resolve that exact tag with `git ls-remote` and present the URL, path, tag,
resolved commit SHA, previous installed version, and advertised version to the
operator. Approval applies only to that evidence. A moving ref, unresolved or
ambiguous tag, version mismatch, unexpected source/path, or evidence change
before the mutation is `BLOCKED`.

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

After each approved change, read back the corresponding JSON list. Never claim
that a marketplace refresh alone changed an installed plugin unless the
installed plugin version or source observed by `codex plugin list --json`
confirms it.

## Native CLI boundary

The plugin may detect a missing `gr` and offer the separate Homebrew protocol
from `install.md`. The plugin installation itself never installs the binary,
and Homebrew installation never registers or installs this Codex plugin. Do
not couple their lifecycle mutations or reuse approval between them.

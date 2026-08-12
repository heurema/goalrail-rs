# Decision 0011: Co-locate the Goalrail Codex plugin

- Status: accepted for remote trial; host reconciliation behavior under trial
- Date: 2026-08-11
- Owner: project owner

## Decision question

Should the Codex-facing Goalrail skill and marketplace live in this repository
or in a separately versioned plugin repository, and should the native package
or the Codex plugin own bootstrap of the other component?

## Constraints and verified facts

- CLI capabilities and agent routing will change together during the current
  product phase.
- A second repository would require synchronization before it provides an
  independent owner, release cadence, or security boundary.
- Codex officially supports a repo marketplace at
  `.agents/plugins/marketplace.json` with plugin paths relative to the same
  repository root.
- A Git-backed marketplace is registered with `codex plugin marketplace add
  owner/repo --ref REF`. Marketplace plugin entries can independently use a
  remote `git-subdir` source and their own `ref` or `sha` selector.
- A minimal plugin may contain only a manifest and skills. It does not require
  an MCP server.
- Installing a Git-backed marketplace creates a refreshable snapshot. Stable
  users therefore need a release ref, not an unbounded assumption that an
  installed cache follows `main` automatically.
- In the current Codex CLI, `marketplace upgrade` refreshes a moving Git
  snapshot and `plugin add` applies its plugin version in place.
- A 2026-08-12 live canary on Codex CLI `0.147.0-alpha.6.5` observed the host
  refresh the Git marketplace and replace the cached Goalrail plugin before a
  newly started agent issued its first command. The agent rollout contained no
  marketplace or plugin mutation command. This is observed host behavior, not
  a documented stable Codex lifecycle guarantee.
- An isolated canary on the same CLI put two separate profiles on a deliberately
  stale Goalrail snapshot, then ran `marketplace list` in one and `plugin list`
  in the other. Both retained the stale snapshot commit and byte-identical
  config. The list commands remain observation surfaces with pre/post evidence;
  this version-scoped result is not generalized into an upstream guarantee.
- Fresh CLI marketplace registration created the Git snapshot and config source
  but no `.codex-marketplace-install.json` or config `last_revision`; the live
  Desktop-managed profile later had both. Update discovery therefore validates
  these fields when present and does not falsely require host-only metadata.
- A canary confirmed that changing a marketplace's own pinned ref is not
  atomic: Codex requires removing the old registration first, and the installed
  plugin is no longer listed while that marketplace is absent.

## Options

1. Keep the skill as a standalone user skill. This is the smallest package but
   gives up marketplace discovery and a coherent plugin update path.
2. Keep a skills-only plugin and repo marketplace beside the CLI. One change
   can update runtime behavior, agent routing, tests, and compatibility notes.
3. Use a separate plugin repository. This separates publication but creates a
   second source that must be synchronized before that separation has value.

For bootstrap ownership:

1. Make Homebrew installation automatically register and install the Codex
   plugin.
2. Install the Codex plugin first; when invoked without `gr`, let its skill
   offer the separately approved Homebrew lifecycle.
3. Keep both packages independent and require the user to discover and install
   each one manually.

## Decision

Use option 2. Store the plugin under `plugins/goalrail` and expose it through
`.agents/plugins/marketplace.json`. Keep `SKILL.md` thin and load detailed
intent routing and package lifecycle instructions from references only when
needed.

The checked-in trial is packaging-only until its exact state exists on GitHub
and the operator separately registers `heurema/goalrail-rs@main` as a Git
marketplace and installs `goalrail@goalrail`. Those are two distinct
configuration changes, each with its own approval and readback.

Use a moving `main` ref for the marketplace catalog so it can advertise new
versions without a remove/re-add gap. The plugin entry itself uses remote
`git-subdir` and points to the immutable shared Goalrail release tag, starting
with `v0.3.0`. The CLI crates and plugin use one version and one tag. A behavior
change in either component advances the whole Goalrail release; offline
validation rejects a mismatched workspace version, plugin manifest, or tag.

Before `plugin add`, the acting agent must read the refreshed marketplace entry
and present its exact URL, subdirectory, `v<version>` tag, resolved tag
commit, and version for approval. A moving ref, mismatched version, unexpected
repository/path, unresolved tag, or changed evidence returns `BLOCKED`. The
installed plugin state must still be verified rather than assumed.
Plugin instructions may call only released CLI capabilities compatible with
the installed binary's `--help` surface. During the remote trial, references may
describe a source-preview command only when they label it unreleased and require
capability detection before invocation. The shared version is a release-train
identity, not proof that a capability exists in an installed binary; runtime
capability detection remains required.

[TRIAL: codex-plugin-host-reconciliation] Update observation records existing
Codex config, optional receipt, snapshot, and plugin cache around the structured
list commands, then compares the snapshot commit with the observed remote
`main` commit. When they differ, the only honest available-version state is
unknown with `STALE_METADATA`; the cached marketplace version cannot justify
`NO_CHANGE`. Starting a task or restarting Codex is not promised as read-only
because the host can reconcile before the skill runs. Manual marketplace
refresh and plugin application remain two separately approved changes. Update
discovery is explicit-intent-only and does not run during ordinary Goalrail
inspection.

Because the moving catalog can be consumed by host reconciliation, `main` must
never advertise a missing payload tag. The release runbook preserves this
ordering: create and verify the immutable tag, publish the release, and only
then promote that exact commit to `main`.

Do not bundle the native `gr` binary in this trial. The documented Homebrew
package remains the single binary distribution and update path. The skill may
offer that path after read-only preflight and exact operator approval.

Select plugin-first bootstrap option 2. The plugin does not silently install
the binary: it detects that `gr` is absent, loads the Homebrew protocol, and
asks for authority for that one package-manager mutation. A Homebrew install,
upgrade, or uninstall must never register, update, or remove Codex marketplaces
or plugins as a side effect. Codex configuration can differ by user and profile
and remains owned by Codex. An explicit future `gr` integration command may be
evaluated if users entering through Homebrew cannot discover the plugin, but it
must not run automatically during package installation.

## Rejected objections

- Co-location alone does not define installed-plugin update semantics. Manual
  `marketplace upgrade` followed by `plugin add` remains a supported explicit
  path, while the current host has also been observed reconciling the catalog
  and installed cache at task startup. Neither observation is generalized into
  an undocumented permanent Codex guarantee.
- A repository marketplace file is packaging metadata, not activation. Users
  register the remote Git marketplace and install the plugin explicitly.
- Pinning the marketplace registration itself to each release tag was rejected
  after a canary showed that changing refs requires temporarily removing the
  marketplace. A moving catalog plus an immutable remote plugin payload keeps
  the update path additive until `plugin add` applies the new version.
- Homebrew post-install is the wrong plugin owner: it would mutate another
  product's user configuration, cannot choose the intended Codex profile, and
  makes independent removal and rollback ambiguous.
- A separate repository does not solve compatibility by itself; it only moves
  the version relation across repositories.
- A bundled executable would reduce the visible install steps but duplicate
  platform packaging, provenance, and update ownership before those contracts
  are proven.
- Matching version text is not sufficient compatibility evidence. The existing
  `v0.2.0` release predates the plugin and remains immutable. Unified versioning
  starts with `v0.3.0`, while the skill still checks the installed help surface
  rather than trusting the version string alone.

## Rollback and revisit

Rollback removes the marketplace entry, plugin folder, and packaging test
without changing the CLI. Revisit a separate repository when the plugin has an
independent maintainer or release cadence, or when its supported surfaces and
capabilities materially extend beyond Goalrail CLI releases.

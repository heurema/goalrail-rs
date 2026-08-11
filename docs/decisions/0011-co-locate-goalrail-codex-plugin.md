# Decision 0011: Co-locate the Goalrail Codex plugin

- Status: accepted for remote trial
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
`git-subdir` and points to an immutable plugin-specific tag from the first
remote canary, starting with `plugin-v0.1.0`. This keeps the code, marketplace,
and release tag in one repository while allowing plugin releases to move more
frequently than native CLI releases.

Before `plugin add`, the acting agent must read the refreshed marketplace entry
and present its exact URL, subdirectory, `plugin-v<version>` tag, resolved tag
commit, and version for approval. A moving ref, mismatched version, unexpected
repository/path, unresolved tag, or changed evidence returns `BLOCKED`. The
installed plugin state must still be verified rather than assumed.
Plugin instructions may call only released CLI capabilities compatible with
the installed binary's `--help` surface. During the remote trial, references may
describe a source-preview command only when they label it unreleased and require
capability detection before invocation. The plugin has its own package version;
changing agent behavior requires a plugin version bump even when the CLI does
not change.

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

- Co-location does not make an installed plugin update automatically. The
  marketplace still requires an explicit refresh followed by `plugin add`,
  with the installed version read back before reporting an update.
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
- Matching Cargo version text is not sufficient compatibility evidence. The
  tagged `v0.2.0` binary predates the plugins drilldown even though the current
  source crate still reports `0.2.0`; the skill therefore checks the installed
  help surface rather than trusting the version string alone.

## Rollback and revisit

Rollback removes the marketplace entry, plugin folder, and packaging test
without changing the CLI. Revisit a separate repository when the plugin has an
independent maintainer or release cadence, or when its supported surfaces and
capabilities materially extend beyond Goalrail CLI releases.

# Make Goalrail Lifecycle Agent-First

- **Status:** adopted; macOS Homebrew lifecycle and multi-platform release
  assets are published
- **Date:** 2026-08-09
- **Last verified:** 2026-08-12 against public `v0.3.6`
- **Decision owner:** t3chn
- **Owner intent:** make Goalrail installation, update, and removal convenient
  for a user's coding agent while preserving explicit user authority and a
  verifiable, fail-closed result

Decision 0012 now has verified public Linux and Windows release assets without
changing this document's public Homebrew lifecycle. Goalrail still provides no
managed package lifecycle for those platforms. Adding one requires a separate
owner decision.

## Context

The primary lifecycle actor is not a person typing package-manager commands.
It is the user's coding agent operating under explicit user authority. The
human approves the operation and receives its result; the agent performs the
preflight, mutation, and verification.

This changes the required UX. A short shell command alone is insufficient. The
agent needs a deterministic protocol that identifies the exact target and
version, refuses ambiguous ownership, performs one bounded mutation, verifies
the postcondition, and returns a structured receipt.

The first concrete environment is macOS arm64 with Homebrew already installed.
The repository currently has no release workflow, package formula, installer,
updater, or lifecycle state. Goalrail does not own Codex configuration or user
project data.

## Options

1. Use Homebrew as the package owner and define an agent-first protocol around
   its native install, upgrade, and uninstall operations.
2. Build a first-party installer, updater, ownership manifest, and uninstaller.
3. Use `cargo install` as the end-user path.

## Decision

Select option 1 for lifecycle v0.

Homebrew owns the installed package and its files. Goalrail does not add
`gr update`, `gr uninstall`, a background update check, a self-modifying binary,
or a second installation database.

The supported v0 target is macOS arm64. The formula is named `goalrail`, the
installed executable is `gr`, and the package source is
`heurema/tap/goalrail`.

## Agent Protocol

Every operation follows the same bounded sequence:

1. Perform read-only preflight.
2. State the exact operation, package, resolved version, and expected path.
3. Obtain explicit user authority for that state-changing operation.
4. Invoke at most one package-manager mutation without an automatic retry.
5. Verify the postcondition independently.
6. Return one structured receipt.

Approval for one operation does not authorize another operation, Homebrew
installation, authentication, privilege escalation, cleanup, or a fallback
installation method.

### Install

Preflight must establish:

- the host is macOS arm64;
- Homebrew is already installed and usable;
- the formula is available and resolves to a concrete version;
- an existing `gr`, if present, is either owned by the same formula or blocks
  installation as ambiguous;
- the expected binary destination is writable through the existing Homebrew
  installation without privilege escalation.

After authority is granted, invoke once:

```bash
brew install heurema/tap/goalrail
```

Verify that Homebrew owns the installed formula, `gr` resolves through that
installation, and `gr --version` matches the installed version.

### Update

Read the installed version and prove that the local Homebrew tap snapshot
matches its observed remote `main` commit before reading the available formula.
`brew info` does not refresh third-party tap metadata. If the commits differ,
return `STALE_METADATA` with an unknown available version; never turn a stale
comparison into `NO_CHANGE`.

Refreshing metadata with `brew update` and upgrading the package are two
separate state-changing actions. Each requires exact authority and its own
readback. After a fresh comparison, if Homebrew reports no outdated Goalrail
formula and the fresh formula version equals the latest public release, return
`NO_CHANGE` without a package mutation. Otherwise, state the installed,
formula, and public-release versions before selecting the update or lag verdict
and obtaining authority for any mutation.

```bash
HOMEBREW_NO_AUTO_UPDATE=1 brew upgrade heurema/tap/goalrail
```

Verify the new Homebrew version, binary path, and `gr --version`.

The same freshness rule applies to the Codex plugin channel: compare the local
`goalrail` marketplace snapshot commit with the observed remote catalog head
before reading its advertised immutable plugin tag. Record existing Codex
config, optional receipt, snapshot, and cache evidence around the structured
list commands. An isolated stale-snapshot canary found the current list commands
left the snapshot and config unchanged. A separate live canary observed Codex
reconcile the marketplace and cached plugin at new-task startup before the
agent's first command, so opening a task or restarting Codex must not be
presented as read-only discovery. The CLI package and plugin remain independent
channels and cannot share mutation approval.

This agent flow runs only on explicit update intent. Goalrail adds no background
network request, daemon, cache, or `gr update` command. Codex host reconciliation
is separate behavior and must be named as such.

For direct macOS, Linux, or Windows release assets, the agent may compare the
installed `gr --version` with the live non-draft GitHub Release only after
requiring the exact supported target asset, checksum, and manifest. This is
discovery, not lifecycle ownership. Linux and Windows have no selected package
manager, installation receipt, or updater, so the agent reports
`MANUAL_REINSTALL_REQUIRED` and never overwrites an ambiguously owned binary.

The live non-draft GitHub Release is also the common version baseline for
managed channels. A fresh Homebrew tap or marketplace that still advertises an
older version is `CHANNEL_LAG` with `WAIT_FOR_CHANNEL` when that channel version
is already installed. If the installation is older still, the channel reports
its intermediate version as `UPDATE_AVAILABLE` with `channelLag: true` and
discloses that waiting avoids a likely second update. A channel newer than the
latest public release is `BLOCKED`. This preserves the release runbook's
separately authorized publication and channel-promotion stages without hiding
their temporary lag from the user.

### Uninstall

Preflight must prove that the target package is installed and that the
resolved `gr` belongs to that Homebrew installation. A missing package returns
`NO_CHANGE`. An unowned or ambiguous `gr` returns `BLOCKED` and is never
deleted.

After authority is granted, invoke once:

```bash
brew uninstall heurema/tap/goalrail
```

Verify that the formula and its package-owned executable are absent. Do not
remove projects, Codex configuration, user configuration, caches, receipts, or
other data outside Homebrew's package ownership.

## Receipt Contract

The acting agent returns one compact object with at least:

```json
{
  "schemaVersion": 1,
  "operation": "install",
  "method": "homebrew",
  "requestedVersion": "latest",
  "previousVersion": null,
  "resolvedVersion": "0.1.0",
  "binaryPath": "/opt/homebrew/bin/gr",
  "verdict": "INSTALLED",
  "evidence": []
}
```

This receipt contract describes completed package lifecycle mutations. The
observation-only update report uses the per-channel discovery verdicts defined
by the Goalrail skill; do not collapse `CHANNEL_LAG`, `STALE_METADATA`, or
`HOST_RECONCILED` into a mutation receipt.

Terminal mutation verdicts are:

- `INSTALLED`;
- `UPDATED`;
- `UNINSTALLED`;
- `NO_CHANGE`;
- `BLOCKED`.

The receipt reports observed facts. It must not contain credentials, arbitrary
environment values, user identity, or unrelated filesystem paths.

## Failure and Fallback Policy

Return `BLOCKED` without mutation when the platform is unsupported, Homebrew is
absent or unusable, package ownership is ambiguous, the resolved version is
unavailable, privilege escalation would be required, or required verification
cannot be completed.

Do not install Homebrew, fall back to `curl | sh`, compile with Cargo, retry a
failed mutation, or switch to another package manager without a separate owner
decision. `cargo install` remains a developer workflow, not a supported
end-user lifecycle path.

## Release Preconditions

The lifecycle protocol does not authorize publication. Before the first public
release, a separate approved release slice must establish:

- selected distribution and licensing terms;
- an immutable version tag and immutable macOS arm64 release asset;
- a SHA-256 produced with the release artifact from the same tag-based macOS
  build;
- a Homebrew formula pinned to that asset and checksum with an explicit
  macOS-arm64 platform guard;
- a checked formula name and `gr` executable-name conflict policy;
- protected write access for the release and tap repositories;
- a tested recovery procedure.

A broken release is not retagged and its asset is not replaced. The v0 recovery
default is a new higher patch release containing the corrective or reverted
code, delivered through the normal Homebrew upgrade path. Self-service
downgrade is not promised until a concrete versioned-formula procedure is
tested.

## Acceptance Fixtures

Before calling the protocol implemented, verify at least:

1. A clean supported host installs the expected version and returns
   `INSTALLED` with matching evidence.
2. The same version already installed returns `NO_CHANGE` without mutation
   only after the fresh managed-channel version also equals the latest public
   release.
3. An available newer version updates once and returns `UPDATED`.
4. An owned installation uninstalls without touching unrelated files and
   returns `UNINSTALLED`.
5. Missing Homebrew, an unsupported platform, a foreign `gr`, and a failed
   ownership proof each return `BLOCKED` without mutation or fallback.
6. A failed package-manager mutation is not retried and cannot produce a
   success verdict.

## Consequences

The product contract is the agent protocol and its evidence, not the brevity of
the underlying shell command. Homebrew supplies package ownership and lifecycle
mechanics, avoiding a Goalrail-specific installer and updater.

The first user without a usable Homebrew path receives an honest `BLOCKED`
result. That case is evidence for revisiting the distribution mechanism, not
permission to add an automatic fallback.

## Operational verification

The second-platform and public-asset revisit completed on 2026-08-12. Native
macOS arm64, Linux x86_64, and Windows x86_64 assets were verified and published
for `v0.3.6`. Keep Homebrew as the only managed lifecycle. Public artifacts do
not expand lifecycle or publication authority and do not imply Linux or Windows
package ownership.

## Revisit Condition

Revisit when a real supported user cannot use Homebrew, a managed lifecycle is
requested for another operating system or architecture, Goalrail begins owning
persistent user state, Homebrew cannot provide the required ownership or
verification evidence, or a stable agent-native package manager satisfies the
same contract with less custom policy.

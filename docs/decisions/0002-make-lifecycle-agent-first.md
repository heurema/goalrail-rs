# Make Goalrail Lifecycle Agent-First

- **Status:** adopted; local packaging is implemented, publication is pending
- **Date:** 2026-08-09
- **Decision owner:** t3chn
- **Owner intent:** make Goalrail installation, update, and removal convenient
  for a user's coding agent while preserving explicit user authority and a
  verifiable, fail-closed result

Decision 0012 adds source-preview Linux and Windows release assets without
changing this document's current public Homebrew lifecycle. A new lifecycle
owner for those platforms requires a separate decision after the first native
runner and public-asset canary.

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

Read the installed and available formula versions first. If they are equal,
return `NO_CHANGE` without mutation. Otherwise, state both versions and obtain
authority before invoking once:

```bash
brew upgrade heurema/tap/goalrail
```

Verify the new Homebrew version, binary path, and `gr --version`.

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

Terminal verdicts are:

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
2. The same version already installed returns `NO_CHANGE` without mutation.
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

## Revisit Condition

Revisit when a real supported user cannot use Homebrew, a second operating
system or architecture is admitted, Goalrail begins owning persistent user
state, Homebrew cannot provide the required ownership or verification
evidence, or a stable agent-native package manager satisfies the same contract
with less custom policy.

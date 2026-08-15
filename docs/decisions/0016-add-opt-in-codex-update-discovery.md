# Decision 0016: Add opt-in Codex update discovery

- **Status:** accepted; implemented locally
- **Date:** 2026-08-15
- **Owner:** project owner

## Decision question

Should Goalrail report newer Codex CLI and desktop app versions, and should
that information change the ordinary Codex inspection verdict?

## Verified facts and constraints

- Ordinary Codex inspection is bounded and local.
- The installed CLI package metadata identifies the official npm package and
  repository.
- The configured npm registry is `registry.npmjs.org`, and local npm
  documentation defines the unqualified package version as the `latest` tag.
- The desktop app exposes an installed version locally, but no official
  machine-readable latest-version source has been verified.
- Discovery does not authorize an update and cannot prove upgrade safety.

## Options considered

1. Add always-on network checks and an upgrade recommendation to ordinary
   inspection.
2. Add a separate opt-in drilldown with per-channel evidence and no risk
   recommendation.
3. Scan repository issues and infer whether an upgrade is safe.

## Decision

Select option 2.

Add `gr inspect codex updates`. Ordinary inspection stays local and exposes the
new command only as a drilldown.

For the CLI channel, query the official npm `latest` metadata and compare
semantic versions. A successful comparison keeps the shared health verdict at
`BASELINE_OK` while reporting `NO_CHANGE` or `UPDATE_AVAILABLE` on the channel.
An installed version newer than the source returns `REVIEW`. A missing CLI,
transport failure, timeout, or invalid version returns `INCOMPLETE` with
`UNKNOWN` availability.

For the desktop app channel, report `UNKNOWN` when the app is installed and
`NOT_INSTALLED` otherwise. Keep source freshness `UNVERIFIED` and do not claim
an available version.

Use the existing `curl` executable with a five-second network timeout and a
ten-second process ceiling. Do not install a client, use npm cache state, launch
an updater, mutate configuration, or fall back to an unverified source.
Resolve `curl` through the operator's `PATH` to preserve the cross-platform CLI
contract and test seam; Goalrail does not claim executable provenance. Report
the registry URL only after the source probe starts. The summary retains the
native `codex --version` text, while the update channel intentionally reports a
normalized semantic version used for comparison.

Treat a version ahead of npm's `latest` dist-tag as `INSTALLED_NEWER` and
`REVIEW`. This is not a downgrade recommendation; it makes prerelease, canary,
or registry-tag divergence visible to the operator.

Reject issue scanning in this slice. General issue activity cannot establish
version-specific upgrade safety without an official advisory contract.

## Rollback and revisit conditions

Remove the drilldown without affecting ordinary inspection. Revisit the app
channel when an official stable machine-readable source is verified. Revisit
risk guidance only if official version-scoped advisories become available.

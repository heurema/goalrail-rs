# Decision 0015: Report the Codex desktop app version

- **Status:** accepted; implemented locally
- **Date:** 2026-08-15
- **Owner:** project owner

## Decision question

Should `gr inspect codex` expose the installed Codex desktop app version
separately from the native Codex CLI version?

## Verified facts and constraints

- `codex --version` reports the CLI version, currently `codex-cli 0.147.0`.
- The installed desktop app is `/Applications/Codex.app`.
- Its app version is the macOS bundle field
  `Contents/Info.plist:CFBundleShortVersionString`, currently `26.715.61943`.
- `Codex.app` may be absent on non-macOS hosts or on hosts using another
  installation layout.
- The existing inspection is read-only and must remain bounded and fail-closed.

## Decision

Add an additive `appVersion` field to the Codex summary. Read the exact local
Codex app bundle with bounded `plutil` execution. Keep `appVersion` as `null`
when the bundle or metadata is unavailable; do not turn optional app metadata
into a failure of the CLI inspection and never substitute the CLI version.

Keep the existing report schema version because this is an additive optional
field and existing consumers can continue reading the current fields.

## Rollback and revisit

Rollback removes the app probe, `appVersion` field, human-output line, tests,
and documentation while leaving the CLI version probe unchanged. Revisit if
Codex changes the desktop bundle location or stops publishing
`CFBundleShortVersionString`.

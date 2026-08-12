# Repository Script Entrypoint Inventory

- Status: verified; no unreferenced executable candidate
- Date: 2026-08-12
- Verification tree: `83d3783c6d3d0ced6bced4c93972170c6abfdae1`
- Scope: tracked executable shell entrypoints in `.githooks/` and `scripts/`
- Mutation authority: none

## Result

The repository contains 27 tracked executable shell entrypoints: one Git hook
and 26 files under `scripts/`. All 27 have a repository-owned entrypoint or
caller. The scan produced:

- `ACTIVE_REFERENCED`: 27;
- `MANUAL_UNKNOWN`: 0;
- `CANDIDATE`: 0.

No file is nominated for archival or deletion. A repository reference proves a
current owned role, not execution frequency, business value, or deletion
safety. External or manual calls remain unobservable, but none is needed to
justify keeping the current set.

## Classification contract

- `ACTIVE_REFERENCED`: invoked or required by a tracked task, workflow, hook,
  build definition, canonical runbook, test, or another active script.
- `MANUAL_UNKNOWN`: no repository-owned caller, but a supported manual or
  external entrypoint may exist. Deletion remains `BLOCKED` until ownership and
  recovery are established.
- `CANDIDATE`: no repository-owned caller or declared manual owner was found.
  This is a review candidate, never deletion authority.

## Inventory

| Entrypoint | Class | Repository-owned evidence |
| --- | --- | --- |
| `.githooks/pre-push` | `ACTIVE_REFERENCED` | Installed by `mise run setup` through `install-git-hooks.sh`; calls `rust-verification-receipts.sh`; covered by the receipt sabotage test. |
| `scripts/architecture-drift.sh` | `ACTIVE_REFERENCED` | Owns `architecture:drift` and `architecture:drift:capture`; exercised by `test-architecture-drift.sh`. |
| `scripts/assemble-release.sh` | `ACTIVE_REFERENCED` | Called by the native release workflow, `release:assemble`, and release-tooling tests. |
| `scripts/build-site.sh` | `ACTIVE_REFERENCED` | Called by the site Docker build and `site:build`. |
| `scripts/check-assessment-boundary.sh` | `ACTIVE_REFERENCED` | Owns `architecture:assessment`; called by the release candidate gate and its sabotage test. |
| `scripts/check-github-release.sh` | `ACTIVE_REFERENCED` | Owns `release:public-check`; required by release preflight and covered by release-tooling tests. |
| `scripts/check-github-run.sh` | `ACTIVE_REFERENCED` | Owns `release:run-check`; called by the canonical release runbook and covered by release-tooling tests. |
| `scripts/check-public-api.sh` | `ACTIVE_REFERENCED` | Owns `architecture:public-api`; exercised by `test-public-api.sh`. |
| `scripts/check-release-bundle.sh` | `ACTIVE_REFERENCED` | Called by the native release workflow, `release:bundle-check`, and release-tooling tests. |
| `scripts/check-release-candidate.sh` | `ACTIVE_REFERENCED` | Called by the native release workflow and owns `release:candidate-check`. |
| `scripts/check-release.sh` | `ACTIVE_REFERENCED` | Called by every native release build, `release:check`, and release-tooling tests. |
| `scripts/check-remote-release-tag-absent.sh` | `ACTIVE_REFERENCED` | Called twice by the native release workflow, by the canonical runbook, and by release-tooling tests. |
| `scripts/check-remote-release-tag.sh` | `ACTIVE_REFERENCED` | Called by the canonical release runbook and covered by release-tooling tests. |
| `scripts/generate-public-api.sh` | `ACTIVE_REFERENCED` | Called by both the public API checker and architecture drift capture. |
| `scripts/install-git-hooks.sh` | `ACTIVE_REFERENCED` | Called by `mise run setup` and covered by the receipt sabotage test. |
| `scripts/package-release.sh` | `ACTIVE_REFERENCED` | Called by `prepare-release.sh`; required by release preflight and covered by release-tooling tests. |
| `scripts/prepare-release.sh` | `ACTIVE_REFERENCED` | Called by every native release build, `release:prepare`, and release-tooling tests. |
| `scripts/release-preflight.sh` | `ACTIVE_REFERENCED` | Owns `release:preflight`, is named by the canonical runbook, and is covered by release-tooling tests. |
| `scripts/rust-verification-receipts.sh` | `ACTIVE_REFERENCED` | Called by the pre-push hook and all three verification-receipt tasks; covered by its sabotage test. |
| `scripts/smoke-goalrail-plugin-remote.sh` | `ACTIVE_REFERENCED` | Owns `smoke:goalrail-plugin-remote`; its contract is checked by `test-goalrail-plugin.sh`. |
| `scripts/smoke-release-binary.sh` | `ACTIVE_REFERENCED` | Called by `prepare-release.sh`; required by release preflight and covered by release-tooling tests. |
| `scripts/test-architecture-drift.sh` | `ACTIVE_REFERENCED` | Owns `test:architecture-drift`. |
| `scripts/test-assessment-boundary.sh` | `ACTIVE_REFERENCED` | Owns `test:assessment-boundary` and is called by the release candidate gate. |
| `scripts/test-goalrail-plugin.sh` | `ACTIVE_REFERENCED` | Owns `test:goalrail-plugin` and is called by the release candidate gate. |
| `scripts/test-public-api.sh` | `ACTIVE_REFERENCED` | Owns `test:public-api`. |
| `scripts/test-release-tooling.sh` | `ACTIVE_REFERENCED` | Owns `release:test` and is called by the release candidate gate. |
| `scripts/test-rust-verification-receipts.sh` | `ACTIVE_REFERENCED` | Owns `test:verification-receipts`. |

## Evidence and limits

The inventory used tracked executable modes, exact-path references in tracked
files, task definitions, workflow and Docker entrypoints, internal calls, and
the Git hook installation path. Research reports were excluded as inbound
reference evidence so this inventory could not make its own entries look
active. `sh -n` or `bash -n` checks syntax only; passing syntax is not evidence
that a script remains necessary.

Repeat this inventory only when a script is added, removed, renamed, or loses
its last repository-owned reference. If a future scan produces a candidate,
record its owner, last known caller, recovery path, and exact verification plan
before proposing any state-changing cleanup.

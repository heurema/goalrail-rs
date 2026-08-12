# Architecture Verification Data

This directory contains machine-readable inputs for narrow architecture checks.
The human-reviewed architecture contract remains discoverable at
[`ARCHITECTURE.md`](../ARCHITECTURE.md).

- `drift/baseline.json` is the accepted identity and size snapshot used by the
  advisory `architecture:drift` task.
- `public-api/gr-inspect-codex.txt` is the pinned rustdoc-visible facade used by
  the `architecture:public-api` trial.

Neither snapshot proves complete architecture conformance. Update one only
after reviewing its diff against `ARCHITECTURE.md`; no check accepts a new
baseline automatically.

# Decision 0010: Link plugin skill evidence in memory

- Status: accepted
- Date: 2026-08-11
- Owner: project owner

## Context

The native plugin inventory and the active skill report exposed separate facts.
An agent could sometimes infer a relation from paths, but the report did not
provide a stable plugin-to-skill link or plugin-level observed-use evidence.
The relationship is useful now; a durable query store is not yet justified.

## Options

1. Keep the lists separate and require callers to infer links. This preserves
   the smallest implementation but does not solve the agent UX problem.
2. Store plugins, skills, links, and observations as normalized Rust records in
   memory, then render a plugin-centric nested report. This adds no durable
   source of truth and is easy to remove or replace.
3. Add SQLite now. This would support repeated cross-entity queries, but would
   introduce schema lifecycle, invalidation, and another state boundary before
   a measured need exists.

## Decision

Use option 2. Keep plugin identity, skill identity `(name, manifestPath)`, the
plugin-skill link, and observed-use evidence as separate internal records. Join
a plugin-origin skill only when its exact manifest path is under exactly one
plugin root. Valid roots are the native plugin `source.path` and the installed
cache root derived from `CODEX_HOME/plugins/cache` plus the native
`(marketplace, name, version)` identity. The second root is required because a
live Codex smoke showed that `source.path` can point at a source/staging tree
while active skills load from the versioned cache. Render linked skills under
each plugin for agent convenience and expose link and history coverage.

The plugin use case reuses the existing assessed skill-evidence pipeline. It
does not parse retained rollouts, reproduce skill assessment, write durable
memory, or make cleanup decisions. Missing, partial, unlinked, or ambiguous
evidence fails visibly; absence of observed skill use is never classified as an
unused plugin because non-skill plugin capabilities are outside this evidence.
Unsafe path components and duplicate plugin IDs also fail visibly instead of
expanding a root or collapsing two records into one identity. The report keeps
per-skill turn counting separate from the plugin-level sum and includes the
assessment timestamp and thresholds required to interpret signals.

## Rejected objections

- A nested response does not require denormalized internal state; it is a
  presentation over normalized records.
- SQLite is not rejected permanently. It is deferred until repeated queries or
  measured runtime demonstrate that rebuilding the relation is inadequate.

## Rollback and revisit

Rollback removes the projection and nested plugin fields while leaving native
plugin and skill probes unchanged. Revisit SQLite after measured repeated-scan
cost, a requirement for historical cross-command queries, or a second consumer
that needs indexed relations across runs.

# Trial Identity-Based Architecture Drift Review

- **Status:** trial
- **Date:** 2026-08-11
- **Decision owner:** t3chn

## Decision Question

How should Goalrail expose cumulative architecture change across individually
reasonable milestones without recreating a source parser or claiming complete
architecture conformance?

## Context

The retired architecture-fitness checker produced one useful signal: repeated
changes had concentrated skill inspection responsibilities in `skills.rs`.
Its implementation was removed because text parsing produced false accepts,
false rejects, and an unsupported Ruby maintenance surface.

The compiler-enforced `gr-skill-assessment` boundary now protects one exact
purity seam. Cargo metadata protects the accepted owned crate edges, and the
separate `cargo-public-api` trial exposes the rustdoc-visible facade. None of
these checks shows their combined movement across several milestones.

## Decision

Add a manually invoked, advisory `architecture:drift` trial. Its accepted
baseline records identities instead of aggregate counts:

- workspace package names;
- owned dependency edges as `(from, to, kind, target, optional)`;
- each item from the pinned current `cargo-public-api` surface;
- each workspace Rust file as `(path, package, line count, content fingerprint)`.

`mise run architecture:drift` compares the current snapshot with
`architecture/drift/baseline.json` and prints structured JSON. `NO_CHANGE`
means those observations are identical. `REVIEW` means at least one exact
identity or Rust file content changed. Both verdicts exit successfully
because drift is evidence for aggregate review, not an architecture violation.
Unavailable or malformed inputs fail non-zero.

The output includes the five largest Rust files so an agent can see current
concentration even when no identity changed. It reports every exact addition,
removal, and changed Rust path with its before/after line counts. A content
fingerprint ensures that an equal-line-count rewrite still enters the aggregate
review scope; one removed observation cannot be silently replaced by a
different observation under the same total count.

`mise run architecture:drift:capture` only prints a candidate snapshot. It
does not update the accepted baseline. Baseline replacement requires an
explicit architecture review and normal reviewed file edit; there is no
automatic accept command. The raw capture has no verdict and must not be used
as an assessment result.

The trial remains outside normal CI. Run it for an aggregate architecture
review after an architecture-affecting milestone or after three related small
milestones.

## Deliberate Limits

- Cargo metadata observes workspace crate edges, not intra-crate dependencies.
- The public API slice inherits the documented `cargo-public-api` omissions.
- File content fingerprints identify review scope, and line counts reveal
  concentration and movement. Neither classifies semantic ownership, coupling,
  duplication, complexity, or an architecture violation.
- The trial does not parse Rust, expand macros, detect intra-crate cycles, or
  prove AD-1 through AD-6.
- A green `NO_CHANGE` must not be described as architecture conformance.

These limits are repeated in every `architecture:drift` check result so an
agent does not need to read this decision before interpreting its verdict.

## Verification

The sabotage harness proves that the command:

- returns `NO_CHANGE` for an identical snapshot;
- identifies reassignment of an unchanged source path to another package;
- identifies same-size source rewrites and source growth by exact path and
  before/after line counts;
- identifies a new owned edge by its complete tuple;
- ignores a registry dependency whose package name collides with a workspace
  package name;
- identifies a new public API item;
- identifies a new Rust file;
- assigns a Rust file in a nested workspace package to the nearest package root;
- rejects invalid or noncanonical baselines, a missing workspace package
  directory, and unguarded input overrides.

The existing public API trial tests remain the regression oracle for the shared
rustdoc snapshot generator.

## Revisit Condition

After three real aggregate architecture reviews or by 2026-09-11, choose
`KEEP`, `MODIFY`, `MOVE`, or `REMOVE`. Revisit immediately after a false accept,
false reject, confusing review, material runtime cost, or pressure to treat
line counts as a semantic architecture rule.

## Rollback

Remove the drift script, sabotage test, baseline, `mise` tasks, this decision,
and its trial documentation. Keep the independent compiler/Cargo boundary and
public API trial.

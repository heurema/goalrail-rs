# Process Trial Evidence

This file records sparse evidence for temporary process, policy, and
configuration trials marked `[TRIAL]` in their owning source of truth. It is
not a transcript or a general activity log.

Record an observation only when a trial:

- changes the next action;
- should have applied but was bypassed;
- causes a review to find a material issue;
- adds noticeable cost or confusion; or
- is modified, moved, or removed.

At the trial's revisit trigger, use the observations to choose exactly one
decision: `KEEP`, `MODIFY`, `MOVE`, or `REMOVE`. While a trial is active, append
new observations. When cleaning it up, either retain one final summary or
remove the evidence with the trial if it has no durable value.

## milestone-closure-gate

- Owner: [`AGENTS.md`](../AGENTS.md#milestone-closure-gate)
- Added: `2026-08-09`
- Revisit: after three milestone closure attempts or by `2026-09-09`, whichever
  comes first.
- Current decision: `KEEP`.
- Closed: `2026-08-09`.

The gate prevented a sibling feature from starting while review remained
unfinished. Independent review then found false-cleanup and path-attribution
risks. Goal-scoped mutation testing exposed missing protection across the
public outcome, catalog probing, rollout aggregation, usage classification,
coverage verdicts, and time parsing. After fixes, the complete milestone diff
passed workspace CI and all 536 generated mutants: 503 were caught and 33 were
unviable, with no missed mutants or timeouts. The repeated mutation cycles were
noticeable but bounded to the milestone diff. Keep the closure gate and its
fail-closed diff requirement as permanent project policy.

## local-verification-receipts

- Owner: [decision 0003](decisions/0003-use-local-verification-receipts-for-push.md)
- Added: `2026-08-10`
- Revisit: after three real pushes or by `2026-09-10`, whichever comes first.
- Current decision: `TRIAL`.

Record only a failed or confusing receipt check, unnecessary verification,
false accept, false reject, material wait, or a change to the trial contract.

The first canary reproduced a false accept: `prek` validated only one range in
a two-ref push, so an unverified Rust branch escaped the hook. Independent
review also found that one state hash could not soundly represent both exact CI
inputs and reusable mutation evidence. The trial now uses a native all-ref hook,
exact full-tree receipts, and CI-only edges only when no Rust source changed.
The final review then rejected both a mutation-input allow-list and public
receipt-only commands. The shipped canary hashes complete Git trees and exposes
only commands that run their checks before writing evidence.

## architecture-fitness-v0

- Owner: [decision 0004](decisions/0004-trial-native-architecture-fitness.md)
  and [`ARCHITECTURE-SPINE.md`](../ARCHITECTURE-SPINE.md#architecture-fitness-v0-trial)
- Added: `2026-08-10`
- Revisit: after three architecture-affecting milestones or by `2026-09-10`,
  whichever comes first.
- Current decision: `TRIAL`.

Record only an architecture violation that changes the implementation, a false
accept, a false reject, an unclear failure, noticeable runtime or maintenance
cost, a manual rule that should become executable, or a check that is modified,
moved, or removed. Routine green runs are not observations.

Baseline observation: AD-1 through AD-5 currently conform, but only the exact
workspace shape, owned dependency graph, source-level facade declarations,
`Verdict` variants, and site owned-dependency isolation are automated. AD-1 and
AD-2 remain manual. `skills.rs` is a cohesion hotspot combining multiple
responsibilities; v0 records that signal without imposing a line-count limit or
claiming an internal boundary that has not been decided.

First canary and review observation (`2026-08-10`): the initial implementation
used a Ruby API unavailable in the supported local runtime, accepted signature
changes (including multiline signatures) and public methods placed after a test
module, and omitted per-AD statuses on failed runs. The checker now uses
compatible iteration, snapshots full literal declarations across every library
source file, prints all AD statuses on both pass and failure, and has regression
fixtures for those failure modes.

Pre-fix trend receipt (`2026-08-10`): the generic canary reports
`crates/gr-inspect-codex/src/skills.rs` as `REVIEW`. Across revisions
`351328b -> b8f5d56 -> 84c9e8e`, source lines grow
`2763 -> 3104 -> 3208` and top-level items grow `81 -> 86 -> 91`; the current
file is above its crate-relative Tukey upper fence of `1717` source lines. This
receipt is evidence of a cumulative hotspot, not a decided module split or an
AD violation. Preserve it unchanged until a later milestone tests a proposed
boundary against the same diagnostic.

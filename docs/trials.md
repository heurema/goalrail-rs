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
  and [`ARCHITECTURE-SPINE.md`](../ARCHITECTURE-SPINE.md#enforcement-status)
- Added: `2026-08-10`
- Closed: `2026-08-11`
- Current decision: `REMOVE`.

The original trend signal was useful: it exposed `skills.rs` as a cumulative
cohesion hotspot and led to the owner-accepted AD-6 evidence-to-assessment
boundary. The first Rust extraction moved the neutral model and pure cleanup
policy without changing public behavior.

The implementation of the fitness gate was not reliable. Repeated reviews found
runtime incompatibility, false accepts, false rejects, and semantic gaps. The
checker grew into a custom Ruby parser for Rust source and compiler output.
Repository-owned Ruby tooling is not an accepted implementation choice for
Goalrail, and this checker did not justify an exception.

The Ruby checkers, fixtures, `mise` tasks, and CI integration were removed. The
Homebrew `.rb` formula remains because Ruby DSL is Homebrew's required package
format and is not part of the architecture trial. CI currently makes no green
architecture-conformance claim.

Before the next AD-6 stage extraction or skills behavior milestone, evaluate a
mature maintained compiler-aware tool against the exact graph and purity rules.
Focused project-specific rules remain allowed when their claims are honestly
scoped and sabotage-tested; do not implement them as repository-owned Ruby.

## cargo-public-api-facade

- Owner: [decision 0006](decisions/0006-trial-cargo-public-api.md)
  and [`ARCHITECTURE-SPINE.md`](../ARCHITECTURE-SPINE.md#enforcement-status)
- Added: `2026-08-11`
- Revisit: after three real facade-change reviews or by `2026-09-11`, whichever
  comes first.
- Current decision: `TRIAL`.

The initial canary caught public additions, signature changes, enum variants,
explicit trait impls, re-exports, macro-generated public items, auto-trait loss,
and `non_exhaustive` removal without reacting to a private-only change. It did
not detect a `#[doc(hidden)] pub` item. The full Goalrail output also exposed 21
unresolved rustdoc references around auto traits, so the accepted task omits
blanket, auto-trait, and auto-derived impls and refuses any remaining unresolved
reference. The trial therefore protects only its checked-in rustdoc-visible
facade slice and remains outside normal CI.

Record a material facade diff found, a false accept, a false reject, confusing
snapshot review, noticeable runtime cost, unresolved rustdoc reference, or a
change to the pinned toolchain, target, feature set, or trial contract.

The first closure review found that the documented setup neither installed the
required nightly toolchain nor pinned the newly required `jq`. The trial was
modified before closure: setup now installs `jq` 1.8.2 and the exact
`nightly-2026-08-07` toolchain, and the checker runs that dated toolchain instead
of the mutable `nightly` alias. The exact toolchain produced the same 24-line
facade snapshot.

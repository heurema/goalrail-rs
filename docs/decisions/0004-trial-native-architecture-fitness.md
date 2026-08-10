# Trial Native Architecture Fitness Checks

- **Status:** trial
- **Date:** 2026-08-10
- **Decision owner:** t3chn
- **Owner intent:** turn the current architecture spine into an executable,
  observable guard before adding another product capability

## Context

The Rust workspace currently conforms to AD-1 through AD-5 in
`ARCHITECTURE-SPINE.md`, but the normal verification path does not test those
decisions as architecture. Rust visibility, Clippy, behavior tests, coverage,
and mutation testing can all pass while an unintended owned dependency edge or
public facade export is introduced.

The recent skills work also concentrated catalog probing, retained-history
evidence, cleanup policy, and presentation in one source module. File length is
only a signal, not an objective architecture violation, and the current spine
does not yet define an internal skills-module dependency rule.

## Options

1. Keep architecture conformance in documentation and manual review. This has
   no tooling cost, but repeats the gap that prompted this decision.
2. Trial a small repository-native check for the current objective invariants,
   expose its incomplete coverage, and collect evidence before extending it.
3. Adopt a broader third-party architecture framework and public-API tool now.
   This can express more rules, but adds installation and maintenance cost
   before the first check has demonstrated useful signal.

## Decision

Trial option 2 as `architecture-fitness-v0`.

`mise run architecture` reads Cargo metadata and fails unless:

- the exact owned workspace members remain `gr`, `gr-inspect-codex`, and
  `gr-site`;
- the exact owned dependency graph remains `gr -> gr-inspect-codex`;
- the normalized full signatures of literal public declarations across the
  library source tree and the `Verdict` variants remain at their reviewed
  baseline.

The check prints an atomic result for each AD and one aggregate result. AD-1
and AD-2 are printed as `MANUAL`: v0 does not claim that source inspection can
reliably prove semantic command and orchestration ownership. AD-5 automation is
limited to the owned-dependency isolation of `gr-site`.

The facade check is a conservative source-declaration snapshot, not semantic
rustdoc API analysis. It inspects literal `pub` declarations in every library
source file, including declarations after test modules. Multiline formatting
or test-only public helpers can therefore require an explicit snapshot update.
A deliberate public contract change must update both the spine and the
snapshot. False accepts or false rejects are trial evidence, not permission to
silently weaken the check.

### Trial extension: historical hotspot review

The hard checks protect known boundaries but cannot diagnose cumulative erosion
that remains legal at each individual change. The first trial extension adds a
separate advisory trend result; it does not redefine a hotspot as an AD
violation.

Three options were considered:

1. Name `skills.rs` or set an absolute line limit. Rejected because it encodes
   the known symptom, is easy to game, and treats size as architecture.
2. Combine a relative structural outlier with repeated historical growth.
   Selected as the smallest generic and reversible canary.
3. Add a semantic architecture framework or infer an internal module split.
   Deferred until the canary identifies a concrete dependency invariant.

The selected signal evaluates Rust source files within each crate. A file is
`REVIEW` only when it is above that crate's Tukey upper fence by source lines
and both its source-line count and top-level item count strictly increase across
the last three revisions that touched it. The three-revision window is a trial
proxy for repeated milestones, not proof that each commit was architecturally
significant. Structural outliers without the repeated-growth evidence are
reported as `OBSERVE`.

The relative fence requires at least four Rust source files and a non-zero
interquartile range within the crate. The source parser counts a literal proxy
of top-level code items: functions, type, trait, implementation, alias,
constant, static, and module declarations. It does not infer semantic
responsibilities. Rename-heavy history, shallow clones, generated source, or
large formatting-only revisions can make the result incomplete or noisy and
must be logged as trial evidence.

`REVIEW` is advisory and does not fail CI because it is evidence of a possible
missing boundary, not an accepted invariant. The hard fitness result and trend
result remain separate so agents cannot interpret a green boundary check as a
claim that the architecture has no hotspots.

### Trial extension: accepted skill pipeline boundary

The hotspot review produced the owner-accepted AD-6 evidence-to-assessment
boundary in [decision 0005](0005-propose-skill-evidence-assessment-boundary.md).
Every hard-check receipt now reports AD-6. Until the Rust modules exist, the
aggregate result is `REVIEW` rather than a misleading green result.

The first canary is intentionally source-level and provisional. It rejects a
partial or unclassified stage topology, pinned forbidden imports, and process,
environment, filesystem, clock, or rendering tokens in assessment. Positive,
lexical and conditional declaration-spoof, forbidden-edge, and per-category
purity fixtures protect those claims. The canary cannot return AD-6 `PASS`; the
first boundary extraction must replace it with a semantic module-graph check
that passes the same sabotage fixtures.

## Constraints and Objections

- A line-count or complexity threshold was rejected because it cannot
  distinguish necessary complexity from poor responsibility boundaries.
- `cargo-public-api` was deferred because its rustdoc snapshot requires a
  pinned nightly toolchain.
- `cargo-modules` was deferred until an internal module boundary exists that a
  cycle check would protect.
- Archaven was deferred because it is new and should be evaluated only against
  a concrete internal dependency rule.
- Green architecture fitness does not replace behavior tests, mutation
  testing, live verification, independent review, or self-review.

## Rollback

Remove the architecture scripts and `mise` tasks, remove the v0 section from
the spine and README, and close or remove the corresponding trial evidence.
No runtime data or migration is involved.

## Revisit Condition

After three architecture-affecting milestones or by 2026-09-10, choose exactly
one of `KEEP`, `MODIFY`, `MOVE`, or `REMOVE`. Consider false accepts, false
rejects, unclear failures, elapsed cost, rules that remained manual, and whether
the skills hotspot produced a concrete internal dependency invariant.

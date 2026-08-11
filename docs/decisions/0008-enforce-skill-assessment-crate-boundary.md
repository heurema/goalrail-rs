# Enforce the Skill Assessment Crate Boundary

- **Status:** adopted trial; implemented and verified
- **Date:** 2026-08-11
- **Decision owner:** t3chn

## Decision Question

How should Goalrail prevent skill assessment policy from accumulating process,
filesystem, environment, clock, presentation, or acquisition dependencies when
source-level architecture linters have demonstrated false accepts?

## Constraints and Verified Facts

- The public CLI, JSON schema, and `gr-inspect-codex` facade must not change.
- The accepted AD-6 direction keeps evidence acquisition policy-free and keeps
  assessment pure.
- The existing `skills/model.rs` and `skills/assessment.rs` extraction already
  identifies the smallest current seam.
- `cargo-pup` 0.1.8 accepted a forbidden qualified path without a `use` item.
- An isolated `archaven` 1.1.0 screen detected that qualified path but accepted
  the same edge when it appeared inside a local `macro_rules!` expansion.
- Rust module visibility cannot grant an orchestrator access to sibling stages
  while reliably denying every sibling-to-sibling reference.
- The change is local and reversible; no deploy, release, public API, or data
  migration is involved.

## Options

### 1. Adopt another source-level dependency lint

Rejected for this boundary. The evaluated tools did not prove the complete
dependency graph, so a green result would overstate conformance.

### 2. Keep assessment as a reviewed module

This remains a valid fallback, but it leaves the exact accumulation risk that
triggered AD-6 to repeated manual review.

### 3. Extract one compiler-enforced internal crate

Selected. Move only the neutral assessment model and pure policy into
`gr-skill-assessment`. Let `gr-inspect-codex` normalize acquired evidence and
depend on the new crate. Do not split catalog, history, presentation, or the
orchestrator in this milestone.

## Decision

Add the unpublished workspace crate `gr-skill-assessment` with these rules:

- it uses `#![no_std]` and `alloc`;
- it may depend only on `serde` with its `std` feature disabled;
- it owns normalized skill evidence, cleanup policy, assessment output, and the
  assessment-specific two-state verdict;
- it does not own process execution, filesystem paths, environment reads,
  clocks, findings, rendering, or the public Goalrail verdict;
- `gr-inspect-codex` converts filesystem paths to normalized strings before the
  boundary and maps the assessment verdict to the existing public `Verdict`;
- Cargo metadata must retain only the AD-3 owned-edge set
  `gr -> gr-inspect-codex` and
  `gr-inspect-codex -> gr-skill-assessment`.

The gate compares the complete workspace owned-edge set against the AD-3
allowlist. It also recompiles the assessment library against a temporary host
sysroot with `std` and `test` removed, so ordinary paths, aliases, comments, and
macro expansion cannot opt the production crate back into `std`.

The compiler and Cargo graph enforce only this extracted boundary. They do not
claim complete AD-6 automation while catalog, history, and presentation remain
in `skills.rs`.

## Rejected Objections

- A new crate adds navigation and manifest overhead. Accepted for this one seam
  because module visibility cannot enforce the required direction and the
  observed hotspot already produced a concrete pure boundary.
- `no_std` requires path evidence to cross as a string rather than `PathBuf`.
  Accepted because assessment treats the value as opaque output data, and its
  serialized representation remains unchanged.
- A custom compiler lint could keep one crate. Rejected for now because it adds
  a nightly compiler-internals maintenance surface without stronger evidence
  than the compiler-enforced package boundary.

## Verification and Stop Condition

The milestone stops after the new crate compiles, Cargo metadata shows only the
accepted owned edges, existing skill behavior and serialized output remain
unchanged, and the repository milestone closure gate passes. Do not extract the
remaining stages in this milestone.

## Rollback

Move the model and assessment code back under `skills/`, restore `PathBuf` at
the internal seam, remove the path dependency and workspace crate, and revert
this decision and the matching spine updates.

## Revisit Condition

Revisit only when another skill stage is extracted, the assessment requires a
new capability, or compiler/Cargo evidence no longer covers the dependency
claim stated here.

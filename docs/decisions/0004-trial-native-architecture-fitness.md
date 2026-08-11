# Retire Custom Architecture Fitness Trial

- **Status:** removed
- **Introduced:** 2026-08-10
- **Removed:** 2026-08-11
- **Decision owner:** t3chn

## Context

The trial attempted to make the architecture spine executable with
repository-owned Ruby scripts. It checked workspace shape, owned dependency
edges, public declarations, source growth, and the proposed AD-6 skills-stage
boundary.

Repeated independent reviews found false accepts, false rejects, runtime
compatibility problems, and gaps between the Rust semantics being claimed and
the source text being inspected. The AD-6 extension accumulated a custom Rust
source parser plus compiler-output interpretation inside Ruby. Repository-owned
Ruby tooling is not an accepted implementation choice for Goalrail, and this
implementation did not justify an exception.

The Homebrew formula is unrelated to this trial. Its `.rb` file remains because
Ruby DSL is Homebrew's required package format.

## Options

1. Continue patching the Ruby parser. Rejected because repository-owned Ruby
   tooling is not accepted and each fix expanded its language-analysis surface
   without establishing soundness.
2. Install a third-party architecture tool immediately. Deferred because no
   candidate has yet proven the complete AD-6 graph and purity requirements.
3. Remove the custom automation now and keep the architecture spine as the
   reviewed source of truth until a suitable tool is selected. Selected.

## Decision

Remove the custom Ruby checkers, their fixtures, and their `mise` and CI tasks.
Do not add other repository-owned Ruby tooling; Homebrew Formula DSL remains
the required exception.

Compiler visibility, Clippy, behavior tests, mutation testing, live smoke, and
review continue to protect the behavior they can actually observe. They do not
claim automated proof of AD-1 through AD-6. Architecture conformance therefore
remains an explicit review responsibility.

Future automation should prefer a mature, maintained, compiler-aware library or
tool selected against a concrete requirement. A focused project-specific rule
is allowed when its claim is honestly scoped. Before CI adoption, every rule
must pass positive and negative sabotage cases for the behavior it claims; a
gate claiming complete AD-6 conformance must prove the exact dependency graph
and assessment-purity rules.

## Consequences

- CI no longer reports a misleading aggregate architecture verdict.
- The useful pre-fix hotspot evidence and accepted AD-6 boundary remain in the
  project record.
- Selecting and adding a replacement dependency is a separate decision, not a
  hidden part of removing the failed trial.

## Rollback

The deleted scripts are recoverable from Git history, but their Ruby
implementation must not be restored as the architecture gate. A replacement
requires a new reviewed decision.

## Revisit Condition

Evaluate mature compiler-aware tools before the next AD-6 stage extraction or
skills behavior milestone. Until then, do not label the architecture automated
checks as passing.

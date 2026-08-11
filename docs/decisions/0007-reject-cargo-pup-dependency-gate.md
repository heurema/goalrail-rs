# Reject cargo-pup as a Dependency-Direction Gate

- **Status:** rejected
- **Date:** 2026-08-11
- **Decision owner:** t3chn

## Decision Question

Can `cargo-pup` 0.1.8 reliably enforce Goalrail module dependency direction,
including the reverse edges forbidden by AD-6?

## Evaluated Candidate

The evaluation used the crates.io release `cargo_pup` 0.1.8 with its required
`nightly-2026-01-22` toolchain and the `rust-src`, `rustc-dev`, and
`llvm-tools-preview` components. The binaries and canary crate were isolated
outside the repository.

Source inspection showed that `RestrictImports` visits only HIR
`ItemKind::Use` items and matches their path segments with regular expressions.
An invalid deny regex is reported to stderr but evaluates as no match.

## Canary Evidence

The canary targeted an `allowed` module and denied imports from a sibling
`forbidden` module.

- `use crate::forbidden::Secret` was rejected with exit code 1.
- The semantically equivalent `crate::forbidden::Secret` qualified path, with
  no `use` item, passed with exit code 0.

The qualified-path false accept is sufficient to reject a dependency-direction
claim. The evaluation stopped at that conclusive result instead of expanding
the matrix after the candidate had already missed the core boundary.

## Decision

Do not add `cargo-pup`, `pup.ron`, setup prerequisites, or a `mise` task to
Goalrail. Version 0.1.8 can lint selected `use` declarations, but it cannot prove
module dependency direction or AD-6 conformance. A green result would therefore
create a misleading architecture signal.

This rejection does not claim that `cargo-pup` is generally defective. Its
`use`-policy lint is simply weaker than Goalrail's required boundary.

## Revisit Condition

Re-evaluate only after an upstream release documents and sabotage-tests
qualified paths as well as `use` declarations, fails closed on invalid rule
configuration, and provides a way to prevent local source attributes from
weakening a repository-owned architecture rule.

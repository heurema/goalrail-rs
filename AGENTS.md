# Goalrail Agent Rules

- Communicate with the owner in Russian.
- Use English for code, comments, filenames, commands, commits, and technical
  documentation.
- Before changing crate boundaries, dependency direction, public APIs, or CLI
  command ownership, read `ARCHITECTURE-SPINE.md` and preserve its invariants.
  If an invariant must change, update the spine before implementation.
- Keep one diff focused on one purpose and preserve unrelated owner work.
- Run `mise run ci` after Rust behavior or architecture changes. Before
  declaring such a milestone `DONE`, create a goal-scoped diff and run
  `GOALRAIL_MUTATION_DIFF=<path> mise run verify:rust-milestone`.
- Do not commit, push, publish, release, deploy, or perform destructive work
  without explicit owner approval.

## Milestone Closure Gate

Adoption evidence is summarized in
[`docs/trials.md#milestone-closure-gate`](docs/trials.md#milestone-closure-gate).

- Treat implementation as `IMPLEMENTED, REVIEW_PENDING` until the milestone
  closure gate passes. Passing tests alone does not close a milestone.
- Before proposing or starting a sibling feature, finish the current milestone:
  - verify the contract and UX against the live target;
  - add targeted and regression tests;
  - for Rust behavior or architecture changes, create a unified diff containing
    only that milestone's Rust changes, including untracked new Rust files,
    then run `GOALRAIL_MUTATION_DIFF=<path> mise run verify:rust-milestone`;
    the command includes workspace CI and mutation testing only for code
    overlapping the supplied diff;
  - obtain one independent review for the aggregate milestone when the change
    is user-facing, architectural, risky, or the owner requested review;
  - perform a separate self-review after the independent review;
  - fix findings or record an explicit owner decision to defer them.
- Before the final verdict, print a closure receipt with exactly these checks:
  `CI`, `Mutation`, `Live smoke`, `Independent review`, and `Self-review`.
  Mark each `PASS`, `NOT_APPLICABLE` with a reason, or `OWNER_DEFERRED` with the
  owner's explicit decision. Any `PENDING`, `FAILED`, or unapproved `DEFERRED`
  keeps the milestone at `IMPLEMENTED, REVIEW_PENDING`.
- End the closure gate with exactly one verdict: `DONE`, `NEEDS_FIX`, or
  `BLOCKED`. Do not begin the next feature before `DONE`.
- When the owner asks "what next?", unfinished closure work takes precedence
  over proposing another feature.
- Minor corrections do not each require a separate independent review when one
  final review covers the complete milestone diff.

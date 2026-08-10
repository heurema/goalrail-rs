# Use Local Verification Receipts for Push

- **Status:** trial
- **Date:** 2026-08-10
- **Decision owner:** t3chn
- **Owner intent:** keep the mutation-tested milestone gate while preventing
  long local checks from holding an idle SSH push connection open

## Context

The pre-push hook currently runs workspace CI and mutation testing after Git
opens the SSH connection. A mutation run can outlive the server's idle timeout,
so all checks pass locally but the push still fails. Repeating the same checks
inside another SSH session adds cost without adding evidence.

The hook must still fail closed. A passing command from an older source state
must not authorize a later Rust change, and several locally verified milestones
may be pushed together.

## Options

1. Keep the slow pre-push checks and add SSH keepalives. This treats the network
   symptom but keeps verification coupled to transport.
2. Run one new full verification immediately before every push and cache only
   the committed `HEAD`. This is simple but duplicates milestone closure work.
3. Record each successful milestone verification locally, bind CI to the exact
   Git tree and mutation evidence to a state transition, then make a native
   pre-push hook validate every pushed ref without running tests.

## Decision

Trial option 3.

`mise run verify:rust-milestone` remains the normal closure command. After CI
and goal-scoped mutation testing pass, it writes a local receipt under the
repository's Git directory. Receipts are never committed.

Each receipt is bound to the exact complete Git tree that was tested. Mutation
evidence is a directed edge from the complete base tree to that verified tree;
there is no allow-list of assumed test inputs. A CI-only edge is permitted only
when the base-to-head diff changes no `.rs` file, matching the previous hook's
mutation trigger. A receipt records:

- the Git base object to which the verified goal diff applies;
- the Git object hash of that diff;
- the Git object hashes of the base and resulting full-tree states;
- the exact CI tree hash;
- the receipt schema and observation time.

The tracked native pre-push hook performs no CI or mutation testing. It reads
every ref update from Git's pre-push input, skips deletions, and requires each
pushed ref tip to have exact CI evidence. It also searches the bounded receipt
graph for a full-tree evidence path from the remote state to the pushed state.
Commit ancestry alone is not treated as verification evidence.

Setup installs an absolute `core.hooksPath` for the clone so linked worktrees
cannot bypass the hook by checking out a revision that predates `.githooks`.
Setup permits the superseded generated `prek` shims but fails instead of
disabling any unrelated executable legacy hook.

Missing or stale evidence fails immediately and prints an exact recovery
command. When an already verified tree has only non-Rust commits after it,
`mise run verify:ci-state -- <base> <head>` adds that CI-only edge. The explicit
fallback for committed work without a complete evidence path is
`mise run verify:outgoing-rust -- <base> <head>`. Both commands run before any
network operation and write evidence only after their checks pass.

## Constraints and Objections

- A receipt is local evidence, not remote CI and not authorization to push.
- The local hook remains editable by the repository owner; receipts do not
  claim a stronger security boundary than the existing hook.
- A hardcoded mutation-input allow-list was rejected because future fixtures,
  workspace members, or tool configuration could fall outside it. Full-tree
  edges preserve fail-closed behavior. A documentation-only commit after a
  verified Rust commit needs only CI, but rebasing the Rust commit onto a
  different tree may require another mutation run.
- `prek` was rejected for this hook because its single ref range can omit a
  second ref in one push. The native hook consumes every Git pre-push input
  line.
- Silently accepting pre-existing commits was rejected because historical test
  output cannot be reconstructed into trustworthy receipts.
- The hook validates pushed ref tips, matching the previous aggregate pre-push
  checks. It does not claim that each intermediate commit is independently
  verified.
- Evidence edges are directional. Reverting or force-pushing to a previously
  verified state can still require a new outgoing verification.

## Rollback

Restore `prek.toml`, its two slow hooks, and the original milestone task; remove
the native hook, installer, receipt scripts, and tasks. Existing local receipts
then become unused and may be deleted from the common Git directory under
`goalrail/`.

## Revisit Condition

After three real pushes or by 2026-09-10, choose `KEEP`, `MODIFY`, or `REMOVE`
using elapsed verification time, stale-receipt failures, false accepts, false
rejects, and whether agents followed the printed recovery command.

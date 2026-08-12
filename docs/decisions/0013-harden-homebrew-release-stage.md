# Decision 0013: Harden the Homebrew release stage

- Status: accepted for three-promotion trial
- Date: 2026-08-12
- Owner: project owner

## Decision question

How should Goalrail prevent repeated operator mistakes when interpreting
Homebrew update state and promoting a verified release formula, without
collapsing the separately approved release stages?

## Evidence

The `v0.3.7` release exposed three different classes of event:

- a concurrent `main` change and transient GitHub runner TLS failures were
  correctly contained by existing identity and digest gates;
- the Homebrew tap used a public HTTPS fetch URL that could not authenticate a
  push, but that was discovered only after a local promotion commit existed;
- `brew outdated --json=v2 <named-formula>` returned valid update JSON with
  exit status `1`, and ad hoc `set -e` and `jq` commands repeatedly treated the
  correct result as failure.

The last two are workflow defects. The release runbook is authoritative, but
too much of its Homebrew stage was reconstructed manually on every release.

## Options

1. Patch only the documentation. Rejected because manual parsing and promotion
   remain the error source.
2. Add a read-only checker and keep formula commit and push manual. Rejected
   because it leaves the most consequential state transition ad hoc.
3. Add two narrow executable contracts: one read-only Homebrew state
   normalizer shipped with the plugin, and one idempotent repository-owned
   Homebrew promotion stage. Selected.

A one-command whole release remains rejected. Candidate publication, workflow
dispatch, tag creation, tag push, draft creation, public release, `main`
promotion, and Homebrew promotion retain separate owner approvals.

## Decision

The bundled diagnostic script is the only Goalrail-owned interpreter of
`brew info` and `brew outdated`. It disables auto-update, accepts only the
documented observed statuses `0` and `1`, validates JSON and status/content
consistency, and emits normalized JSON. Status `1` is success only when the
named Goalrail formula is actually reported outdated. The diagnostic never
refreshes metadata or changes an installation.

The promotion script consumes one exact published release bundle and one tap
checkout. Before editing it verifies the release tag, selected run and attempt,
bundle, public archive digest, tap repository identity, current remote head,
and canonical GitHub write permission. It probes the exact canonical SSH push
transport with a non-interactive dry run before creating a commit. It never
uses `brew`, updates the local Goalrail installation, or changes another
release stage.

Promotion has three terminal outcomes:

- `PUBLISHED`: one exact formula commit was pushed and verified through Git
  object identity;
- `NO_CHANGE`: the formula at the remote head is byte-identical to the verified
  release formula;
- `CONFLICT`: version, content, repository, identity, local state, remote head,
  or resume evidence differs from the exact contract.

The script may resume only an exact prepared commit whose parent is still the
observed remote head, whose tree contains the recomputed formula bytes, and
whose diff changes only `Formula/goalrail.rb`. It never resets, rebases,
force-pushes, or retries after a remote race. Commit author and committer are
pinned to the verified release commit's GitHub noreply identity and signing is
disabled for this non-interactive mechanical commit.

Post-push verification uses the remote Git head and blob, not a CDN-cached raw
URL. Formula version equality without byte equality is `CONFLICT`, never
`NO_CHANGE`. Publication rollback remains a higher Goalrail version.

## Independent critique

An independent authority review returned `MODIFY`. It required a write-capable
preflight rather than SSH read access, byte-exact `NO_CHANGE`, Git-object
post-verification, one normalized brew boundary, a defined interrupted-state
machine, non-interactive commit identity, public-archive digest recomputation,
and sabotage cases for rejected pushes and conflicting state. Those objections
are adopted. The script uses both GitHub repository permission evidence and a
dry-run of the exact SSH transport before local mutation.

Critique receipt: requested and actual model `claude-fable-5`, status
`success`, no fallback, exposed cost `$0.421470`.

## Authority and revisit

Running the promotion script remains an external action and requires explicit
owner approval for the exact version and tap. Passing preflight never grants
that authority. Revisit after three real promotions, or immediately after any
false accept, false reject, confusing recovery, credential ambiguity, or
material operator-cost complaint.

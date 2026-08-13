# Add a Claude Inspection Crate Beside the Codex One

- **Status:** adopted; implemented and verified
- **Date:** 2026-08-13
- **Decision owner:** t3chn

## Decision Question

Goalrail inspects a Codex installation. How should it inspect a local Claude
Code installation without weakening the AD-1 through AD-6 spine, without
duplicating the bounded process runner, and without claiming diagnostics that
Claude Code does not expose?

## Constraints and Verified Facts

Verified against the Claude Code documentation, then against the locally
installed CLI (2.1.226) on 2026-08-13:

- `claude --version`, `claude plugin list --json`, and
  `claude plugin marketplace list --json` are documented machine-readable
  surfaces. `claude plugin list --json` reports `id`, `version`, `scope`,
  `enabled`, `installPath`, and install timestamps.
- `claude doctor` prints diagnostics as text only. There is no `--json` form and
  no schema-versioned check map, so the Codex `doctor` gate has no analog.
- Claude Code has no `features list` analog.
- `claude mcp list` has no `--json` flag and health-checks every server, so it is
  neither machine-readable nor read-only in effect. MCP servers are documented as
  living in `~/.claude.json` (user and per-project local scope) and in a project
  `.mcp.json`.
- There is no CLI that lists skills. Skills are documented at
  `~/.claude/skills/<name>/SKILL.md` and `.claude/skills/<name>/SKILL.md`.
- `CLAUDE_CONFIG_DIR` relocates every `~/.claude` path, including the global
  `.claude.json` that tenants must not share.
- Claude Code transcripts under `~/.claude/projects/` record a skill invocation
  by name, not by manifest path, so the Codex evidence basis
  `exact_skill_manifest_path_in_retained_rollout_tool_calls` has no equivalent.
  Usage history is therefore out of scope for this milestone.
- `Verdict` and the bounded process runner are needed by both inspection
  libraries; `run_bounded` is 130 lines of subprocess, timeout, and drain logic
  that must not exist twice.

## Options

### 1. Add Claude inspection inside `gr-inspect-codex`

Rejected. The crate name would stop describing its contents, and AD-2 and AD-4
bind that crate to Codex inspection ownership and its Codex facade.

### 2. Add `gr-inspect-claude` that depends on `gr-inspect-codex`

Rejected. It reverses nothing in Cargo terms but couples the Claude report to a
Codex facade and forces `run_bounded` to become public there, which AD-4
forbids.

### 3. Duplicate `Verdict` and the process runner in a standalone crate

Rejected. It creates a second source of truth for exit codes and for timeout and
drain behavior.

### 4. Extract `gr-inspect-core` and add `gr-inspect-claude` beside the Codex crate

Selected. The second consumer is what proves the shared seam, and the extraction
is a move rather than a redesign: the runner and its tests transfer unchanged.

## Decision

Add two unpublished workspace crates:

- `gr-inspect-core` owns the bounded process runner, the public `Verdict`, and
  exit-code formatting. It names no agent and depends on no workspace crate
  (AD-7). Each adapter imports `Verdict` from the core directly.
- `gr-inspect-claude` owns the `gr inspect claude` summary: probe sequencing,
  failure classification, report synthesis, and rendering (AD-2, AD-4).

AD-3 becomes an exact six-edge set, and `architecture:assessment` compares that
set in CI, so an unapproved edge fails the build rather than a review.

AD-8 binds the evidence surface: only the three native commands above and the
documented configuration paths under the resolved Claude home. Where Claude Code
exposes no machine-readable surface, the report names the gap in
`evidenceLimitations` rather than parsing human output, guessing an undocumented
file, or dropping the subject silently. The command writes nothing.

Verdicts keep their existing meanings. `REVIEW` is reserved for two observable
inconsistencies in the native inventory: an installed plugin whose recorded
`installPath` does not exist, and a plugin id reported more than once. Absent
configuration is evidence of zero, not failure; malformed configuration and every
failed, timed-out, or unparsable probe are `INCOMPLETE`.

## Rejected Objections

- *Parse `claude doctor` text so the summary has a health signal.* Rejected. The
  output is undocumented and unversioned, and a parsed status would look like the
  schema-checked Codex `doctor` evidence while being a guess.
- *Read `~/.claude/plugins/installed_plugins.json` directly and skip the
  subprocess.* Rejected. That file is not documented; `claude plugin list --json`
  is the surface Claude Code commits to.
- *Report project trust like the Codex summary does.* Rejected. The documentation
  states that `~/.claude.json` tracks trust-dialog acceptance but does not name the
  field, so the report states only whether a project state entry exists.
- *Count plugin-contributed skills from each `installPath`.* Deferred. Plugins may
  declare custom skill directories or a root `SKILL.md`, so a directory scan would
  undercount. The limitation is stated instead.

## Independent Review Dispositions

One independent review ran in a fresh Codex session on 2026-08-13 and returned
`REJECT` with no P1, six P2, and two P3 findings. Dispositions:

- **Accepted and fixed.** The `projects` key lookup was keyed on the current
  directory while `project.root` reported the repository root, so a reader could
  misattribute `stateEntry` and the local MCP count. The report now carries
  `currentDir` and the exact matched `stateEntryPath`.
- **Accepted and fixed.** `{"mcpServers": {"broken": null}}` parsed through
  `IgnoredAny` and counted as a configured server. Server entries must now
  deserialize as JSON objects; their values are still discarded unread.
- **Accepted and fixed.** `Path::exists` turned an unreadable project root
  marker, skill manifest, or plugin install path into a silent negative. Root
  discovery and manifest counting now use `try_exists`/`fs::metadata` and fail
  closed, and an unexaminable install path produces the new
  `plugins.install_path_unreadable` finding.
- **Accepted and fixed.** Human output called instruction sources `active` while
  the code observes only existence and size. They are now `discovered`, in the
  command output and in the README.
- **Accepted and fixed.** The instruction-discovery test asserted scopes only. It
  now asserts the complete ordered path list across a nested working directory.
- **Rejected with reason.** Failure reports omit `evidenceLimitations`. The
  limitations describe the coverage of a completed summary; a failure report
  states why no summary exists. Adding them would also diverge from the Codex
  failure-report contract, which carries findings only.
- **Accepted and fixed, one half.** The review pointed at the output drain, and
  running the moved runner under workspace coverage exposed a real starvation
  bug there: both streams shared one absolute deadline, so a first read that
  consumed the whole budget made the second stream return as if the process had
  written nothing to it. This reproduced as a failing
  `captures_stdout_stderr_and_exit_code` with an empty `stderr`. Each stream now
  gets its own drain window, bounded at the timeout plus one grace period per
  stream, with a regression test that starves the first reader deliberately.
- **Deferred, owner-approved on 2026-08-13.** A descendant process holding the
  output pipe still keeps `run_bounded` waiting for the full probe timeout and
  leaves its reader threads blocked. That behaviour is pre-existing, identical on
  the Codex path, and bounded by an existing test. Fixing it means process-group
  termination and joined reader handles, which is its own milestone. The owner
  accepted the deferral explicitly rather than holding this milestone for it.

### Second round

A second independent Codex review read the complete diff and returned `REJECT`
with no P1, five P2, and one P3. Dispositions:

- **Accepted and fixed.** This decision still called AD-3 a five-edge set after
  the `gr -> gr-inspect-core` edge was added, so the gate and the contract
  disagreed. Both statements now say six.
- **Accepted and fixed.** The README claimed that an unreadable skill manifest
  or instruction file fails the inspection. It does not: `fs::metadata` succeeds
  on a mode-`000` file, and a dangling symlink resolves to absent. The command
  examines metadata and never opens contents, and the README now says exactly
  that. The claim was wrong, not the code.
- **Accepted and fixed.** A working directory with no `.git` marker skipped its
  `.mcp.json` entirely, because the project MCP path was `None` without a
  project root while skills and instructions already fell back to the current
  directory. The MCP path now uses the same fallback.
- **Accepted and fixed.** The fake `claude` executable in the CLI tests matched
  on `$1` only, so a probe that called an undocumented command or dropped
  `--json` would still have been answered with valid JSON. It now matches the
  complete argv, fails loudly on anything else, and records each invocation; a
  new test asserts that exactly the three documented commands run. This turns
  AD-8's evidence-surface claim into a checked one.
- **Already resolved.** The finding that `gr_inspect_claude::Verdict` remains
  publicly exported described a tree that changed while the review was running;
  the re-export was removed with the public-API fix below.
- **Rejected with reason, residual risk accepted.** Duplicate JSON keys in
  `.claude.json` are resolved last-wins by `serde_json` rather than reported as
  ambiguous. A duplicate-key-detecting deserializer is real complexity for a file
  written by Claude Code itself, where a duplicate key would be an upstream
  defect. The gap is recorded here rather than masked.

## The Verdict Re-export, and Why It Was Removed

The first implementation kept `pub use gr_inspect_core::Verdict;` in
`gr-inspect-codex` so that its facade would look unchanged. Running the manual
`architecture:public-api` trial rejected that: `cargo-public-api` reported
`rustdoc JSON missing referenced item` for both crates carrying the re-export,
while `gr-inspect-core` itself generated cleanly. A cross-crate `pub use`
produces a reference the per-package rustdoc JSON cannot resolve, so the trial
fails closed rather than pinning a partial facade.

`Verdict` is therefore exported by `gr-inspect-core` alone, each library keeps a
`pub(crate)` alias only where its own modules already say `crate::Verdict`, and
`gr` imports `Verdict` from the core. That adds the owned edge
`gr -> gr-inspect-core`, recorded in AD-3 and in the CI gate.

This is a deliberate change to the `gr-inspect-codex` *library* API: `Verdict`
is no longer reachable as `gr_inspect_codex::Verdict`, and the pinned snapshot
drops from 24 to 17 items. The *observable* contract — JSON fields, human
output, and exit codes — is unchanged, and the 103 `gr-inspect-codex` unit tests
and 11 CLI integration tests that assert exact strings and exit codes all pass
unmodified.

## Verification and Stop Condition

The milestone stops after the workspace compiles, `architecture:assessment`
accepts exactly the six owned edges, unit tests cover every probe and
configuration boundary, CLI integration tests cover all four verdicts and prove
the run writes nothing to the Claude home, `mise run ci` passes, and diff-scoped
mutation testing passes. Do not add skills or plugins drilldowns in this
milestone.

## Rollback

Remove `gr-inspect-claude`, the CLI subcommand, and its tests; move
`process.rs` and `Verdict` back into `gr-inspect-codex`; restore the two-edge
AD-3 set in the spine and in `scripts/check-assessment-boundary.sh`; revert this
decision.

## Revisit Condition

Revisit when Claude Code exposes a machine-readable diagnostic or skills surface,
when a usage-history basis becomes available that does not rest on skill names
alone, when a third inspection library appears, or when the plugin JSON contract
changes.

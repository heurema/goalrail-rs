# Historical Candidate: `gr inspect codex` v0

- **Status:** historical design input; not an active contract
- **Original date:** 2026-08-08
- **Language:** Rust
- **Superseded by:** the current README, runtime behavior, and deterministic
  tests

## Preservation Note

The candidate below is retained because it records useful early boundaries,
but it was never adopted verbatim. Known differences in the implemented slice
include:

- `BLOCKED` uses exit code `4`, not `2`;
- JSON reports use numeric `schemaVersion: 1`, not the proposed schema ID;
- skill discovery, content digests, coverage categories, and conflict analysis
  are not implemented;
- current behavior is defined by the checked-in code and tests, not this file.

The remaining text is preserved as historical design evidence.

## Purpose

`gr inspect codex` produces one bounded factual baseline of the Codex
environment in the current directory before Goalrail initialization. It does
not modify Codex, inspect sessions, judge natural-language semantics, or
certify Goalrail compatibility.

## Command

```text
gr inspect codex [--json]
```

Human output is the default. `--json` emits one versioned object and no prose.

## Native Evidence

| Probe | Command | Treatment |
|---|---|---|
| version | `codex --version` | required |
| doctor | `codex doctor --json` | required |
| features | `codex features list` | best effort, `PARTIAL` |
| plugins | `codex plugin list --json` | required when supported |
| marketplaces | `codex plugin marketplace list --json` | required when supported |
| MCP | `codex mcp list --json` | required when supported |

Run each probe once, without retry. Limit each probe to 15 seconds and the whole
command to 60 seconds. Capture output streams, exit status, duration, and native
schema identity. Text-only feature output may produce facts but cannot alone
produce `REVIEW` or `BLOCKED`.

## Narrow Local Metadata

Fill only demonstrated native gaps:

- the effective instruction chain for the current directory: the user-global
  `AGENTS.override.md` or `AGENTS.md`, then one applicable instruction file per
  directory from the repository root to the current directory, using documented
  Codex precedence; record each path, scope, size, and digest;
- discovered skill name, description, scope, source, and enabled state when
  determinable from documented configuration;
- manifests for plugins already returned by the native plugin list.

Do not emit instruction or skill bodies. Do not inspect hooks, unrelated
directories, sessions, prompts, or arbitrary plugin code.

## Evaluation

Category coverage is `OBSERVED`, `PARTIAL`, or `UNOBSERVED`.

- `BASELINE_OK`: required evidence is observed with no actionable issue;
- `REVIEW`: an exact overlap, documented loading-limit breach, or relevant
  partial coverage needs owner inspection;
- `BLOCKED`: a required Codex install, config, runtime, or sandbox dependency
  explicitly failed;
- `INCOMPLETE`: required evidence is unavailable, timed out, malformed, or
  uses an unsupported schema.

The doctor aggregate status alone never decides the verdict. Normal
`AGENTS.md` precedence is not a conflict. Duplicate identifiers are exact
overlaps, not proven harm. Custom sources, unstable features, and unused MCP
authentication state remain facts unless they affect a required v0 dependency.
Malformed evidence reduces coverage; it is not a configuration conflict.

## Output Contract

Schema `goalrail.inspect.codex/v0` contains the redacted target and Codex
version, verdict, coverage, probe status and duration, native schema identity,
sanitized facts and findings, evidence references, and non-mutating next actions.

Exit codes: `0 BASELINE_OK`, `1 REVIEW`, `2 BLOCKED`, `3 INCOMPLETE`,
`64 invalid usage or internal contract violation`.

## Boundary

Use a structural output allowlist. Never emit secrets, environment values, raw
MCP arguments, prompt text, instruction bodies, or unredacted home paths.
Goalrail writes no files and invokes no model. Native diagnostics may perform
their documented checks, including network reachability.

## Acceptance Fixtures

1. Healthy supported evidence produces `BASELINE_OK`.
2. A critical required native failure produces `BLOCKED`; doctor
   invocation-context noise does not.
3. Missing, timed-out, malformed, or unsupported required output produces
   `INCOMPLETE`; unknown JSON fields remain forward-compatible.
4. Normal nested `AGENTS.md` precedence produces no finding; an exact duplicate
   active identifier produces `REVIEW` without claiming semantic conflict.
5. Both output modes pass the structural allowlist; Goalrail performs no
   filesystem write or model call.

## Deferred

Hooks, semantic conflict analysis, repair, session analysis, Claude inspection,
continuous monitoring, initialization, and Goalrail-specific compatibility
rules. Compatibility rules begin only when a real Goalrail integration declares
its concrete footprint.

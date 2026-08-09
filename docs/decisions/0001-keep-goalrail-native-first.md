# Keep Goalrail Native-First

- **Status:** adopted; the first read-only Codex adapter is implemented
- **Date:** 2026-08-08
- **Decision owner:** t3chn
- **Owner intent:** prevent the new Goalrail from growing beyond validated need,
  prefer supported native agent capabilities whenever they are no worse, and
  audit the current agent environment before Goalrail is activated in a project

## Context

The current Goalrail grew from a small harness into setup, update, overlay,
migration, admission, lineage, review, evidence, and runtime machinery before
it had users. The reset must prevent the same expansion rather than merely
produce a smaller first implementation.

Goalrail also cannot assume that a local coding-agent environment is coherent.
A project may inherit conflicting `AGENTS.md` instructions, skills, plugins,
MCP servers, hooks, or feature flags. In that state Goalrail may add little
value or behave unpredictably even when Goalrail itself is correct.

Codex exposes the relevant evidence through several native commands, but no
single command answers whether the combined local environment is suitable for
Goalrail. Requiring the user to run and compare every command manually would
move the integration burden to the user.

## Options

1. Preserve the current broad harness and remove modules incrementally. This
   retains unvalidated boundaries and makes compatibility work the first
   concern of a product with no users.
2. Document the native commands and require users to run and interpret them.
   This adds no code, but does not provide a useful preflight experience.
3. Make the first Goalrail capability a read-only audit that orchestrates
   native Codex commands and analyzes their combined evidence without replacing
   the native checks.

## Decision

Select option 3.

The first user-visible command is `gr inspect codex`. It runs the smallest
supported native command set needed to inspect the current environment:

- `codex --version`;
- `codex doctor --json`;
- `codex features list`;
- `codex plugin list --json`;
- `codex plugin marketplace list --json`;
- `codex mcp list --json`.

Experimental debug surfaces that expose raw model-visible content are excluded
from the first contract.

The first implementation correlates native installation and runtime health,
doctor findings, feature rows, installed plugins and marketplaces, configured
MCP servers, project trust, and model-visible instruction sources. Conflict,
duplication, skill, and context-pressure analysis are not implied by this
decision; each requires a separate admitted capability. Goalrail does not
install, remove, enable, disable, repair, authenticate, or rewrite
configuration.

Goalrail remains an external harness, not an owner of the agent scaffold. The
project and its `AGENTS.md` remain project-owned. Codex configuration, plugin
lifecycle, hooks, sandbox, approvals, and execution remain Codex- and
user-owned.

## Capability Admission Rule

A capability enters Goalrail only when all of the following exist:

1. A named consumer and the concrete decision or outcome the capability changes.
2. A reproducible gap in the current documented stable native surface, or a
   demonstrated need to correlate multiple native surfaces for that decision.
3. The smallest acceptance fixture, including authority, privacy, and failure
   behavior.
4. An explicit statement of owned state and possible side effects.
5. A native-parity test and a deletion trigger defined before implementation.

If any item is missing, the result is `NO_BUILD`.

One owner-approved change may add one bounded capability. It must not introduce
a generic registry, compatibility layer, workflow engine, daemon, database,
dashboard, installer, updater, or provider abstraction unless that same
capability requires it and the acceptance fixture proves the need.

## Native Adoption and Deletion

- Stable native features are replacement candidates. Beta features may run in
  an isolated canary. Experimental or under-development features cannot become
  required dependencies.
- Compare native and custom behavior on the same frozen fixture. Correctness,
  authority and safety, privacy, structured output, observability, and recovery
  are hard requirements. Unknown evidence blocks replacement; it is not parity.
- Capability discovery may be automatic, but promotion to the native path is an
  explicit tested change, never an unreviewed effect of upgrading Codex.
- When native behavior is no worse on every hard requirement, switch to native
  and delete the custom path. Do not keep permanent dual-run, dual-write, or
  fallback behavior.
- While Goalrail has no users, Git is the rollback mechanism; no compatibility
  window or migration layer is justified.

## Integration Boundary

Use native extension points rather than modifying a scaffold:

- start with a skill when instructions and existing tools are sufficient;
- add an MCP server only when a missing controlled tool is demonstrated;
- add hooks only through explicit, reversible, trusted opt-in;
- package an established capability as a plugin, but leave installation,
  enablement, disablement, permissions, and updates to the native plugin manager.

Start with one Codex-specific adapter. A separate Claude inspection may be
added only after the Codex command is useful and its contract is stable.
Generalize only after the second provider demonstrates a second concrete shape.

Session analysis is a separate later capability. It may examine how the user is
actually working only after the local Codex environment audit is understood.
It is not part of `gr inspect codex` and does not inherit authority to read,
retain, or summarize session history.

The first implementation uses Rust by owner decision. Start with one small CLI
binary and add only the dependencies required by the accepted command contract
and fixtures. The language choice must not justify a framework, daemon, service,
or broader runtime before the first slice proves the need.

## Evidence

- OpenAI documents `codex doctor` and `codex exec` as stable CLI capabilities:
  <https://learn.chatgpt.com/docs/developer-commands?surface=cli>.
- OpenAI feature maturity distinguishes Stable, Beta, Experimental, and Under
  development adoption expectations:
  <https://learn.chatgpt.com/docs/feature-maturity>.
- OpenAI plugin architecture recommends the smallest plugin shape and supports
  skills, MCP servers, or both:
  <https://developers.openai.com/plugins/concepts/plugins>.
- One isolated Claude Fable critique correctly rejected a wrapper around
  `codex doctor` alone. This decision accepts that objection: the value of
  `gr inspect codex` is correlation across multiple native surfaces, not a
  renamed doctor report.

## Consequences

The first slice has one purpose: produce a trustworthy factual Codex baseline
before Goalrail adds an integration surface. It does not certify Goalrail
compatibility, analyze session history, modify the environment, or initialize
Goalrail in a project.

## Revisit Condition

Revisit this decision when the Codex audit contract is proven on real local
configurations, a second provider requires a materially different adapter, the
first external user creates a compatibility obligation, or one stable native
command can replace the combined Goalrail audit without losing required
cross-surface evidence.

# Decision 0017: Trial chat-native development intent

- **Status:** accepted as an isolated evaluation fixture; live canary pending
- **Date:** 2026-08-17
- **Owner:** project owner

## Decision question

Should Goalrail improve software-development intent through a Rust workflow,
the existing inspection skill, a separate native Codex skill, or an external
orchestrator?

## Constraints and verified facts

- The ordinary Codex chat should remain the user interface and execution
  runtime. Codex already owns tools, tasks, context, models, reasoning effort,
  subagents, permissions, and execution.
- Decision 0001 keeps Goalrail outside the agent scaffold and admits a custom
  capability only after a gap in the stable native surface is demonstrated.
- Decision 0011 already co-locates the Goalrail Codex plugin in this repository
  and keeps its skills progressively disclosed.
- The current `goalrail` skill owns environment inspection and package/plugin
  lifecycle. Adding development intent to that trigger would mix unrelated
  workflows and make ordinary Goalrail requests ambiguous.
- Model names and available effort levels change. A product path that hard-codes
  one model-role chain would become stale and would claim routing quality before
  Goalrail has controlled evaluation evidence.
- The project has a deferred receipt comparator proposal, but no accepted
  runtime, provider adapter, or model router.
- No reproducible case currently proves that native Codex plus project
  instructions cannot produce the desired intent behavior. The reported long
  chat and same-agent concerns are trial hypotheses, not capability-admission
  evidence.

## Options considered

1. Add a Rust pipeline that selects models, sequences roles, and manages
   sessions. This duplicates native Codex ownership and creates a workflow
   engine before a reproducible gap exists.
2. Expand the existing `goalrail` skill. This is small but couples development
   behavior to environment inspection and lifecycle intent.
3. Develop one focused high-freedom `goalrail-intent` skill as an isolated
   evaluation fixture. Keep execution native and package the skill only after a
   reproducible native gap and acceptance evidence satisfy decision 0001.
4. Build a separate service or repository. This adds distribution, state,
   credentials, and synchronization boundaries without a distinct owner.

## Decision

Select option 3 for a bounded trial.

Store the candidate under
`fixtures/chat-native-development-intent/goalrail-intent`, outside
`plugins/goalrail`. The fixture is not part of the released plugin, is not
advertised by its manifest or marketplace entry, and is not an admitted
Goalrail capability. A live canary may load this exact fixture through a
separately approved temporary local test setup; installation does not promote
it into the product.

The skill starts from ordinary user language and the current project. It reads
the applicable project evidence, distinguishes routine work from a
decision-bearing change, and returns routine work to normal Codex execution
without mandatory research or artifacts.

For architecture, public API, ownership, security, privacy, cross-cutting, or
otherwise high-impact decisions, require comparison of two or three current
viable approaches before inventing a mechanism. The native or no-build path is
a candidate. Prefer project precedent, official documentation, mature
maintained implementations, and authoritative research. Preserve conflicts and
unknowns instead of converting weak evidence into certainty.

The skill may guide Codex to choose tools, split context, start a fresh task, or
request independent review when current authority and project instructions
permit it. It must not fix a model, effort, role chain, or agent count, and it
must not own execution. High-impact owner decisions and external or irreversible
actions remain approval-gated.

Persist only durable owner-confirmed decisions, using the source of truth the
project already owns. OpenSpec is an optional project adapter, not a Goalrail
dependency or universal source of truth.

This change does not add a packaged plugin skill, Rust command, crate, MCP
server, hook, database, provider integration, session store, or model router.
It does not install or reload the local plugin and does not change the released
manifest or marketplace target.

## Trial contract

The first live canary has a hard ceiling of five attempted real
software-change pairs and no automatic second round. Compare ordinary Codex
behavior with behavior when the fixture is loaded from the same frozen starting
state. Fixture availability is the only changed dimension in each pair: use the
same exact model, effort, tools, permissions, budgets, acceptance checks, and
attempt ceiling, and counterbalance execution order across the case set. Select
the cases and their acceptance checks before either run. Treat this as a
feasibility canary, not a promotion-quality model ranking.

For each case, retain:

- the user intent and frozen repository identity;
- the deterministic acceptance and forbidden-change checks;
- verified actual model, effort, tools, permissions, and fixture availability;
- whether useful existing approaches were found;
- material owner corrections, duration, and exposed usage or cost;
- the resulting diff, checks, review, and terminal outcome.

Preflight each pair before execution. If the host cannot expose and bind every
frozen dimension, mark the pair `INVALID`, do not run or count it, and consume
one of the five attempts. Any invalid pair prevents `KEEP` in this trial. Stop
the canary immediately after an authority, security, privacy, or false success
regression. Also stop and return `MODIFY` after two clear routine tasks are
unnecessarily escalated into research or decision packets. At the attempt
ceiling, choose exactly one verdict: `MODIFY` or `REMOVE` when any pair is
invalid; otherwise choose `KEEP`, `MODIFY`, or `REMOVE`.

Do not infer a model-routing policy from this trial. A later controlled routing
experiment must change exactly one model or effort dimension and satisfy the
comparison invariants in `docs/ideas/model-behavior-evaluation.md`.

## Rollback and revisit

Rollback removes the isolated `goalrail-intent` fixture, its focused contract
test, and this trial record without changing the CLI or released plugin. Admit
the skill into `plugins/goalrail` only after the canary demonstrates a
reproducible native gap, decision 0001's remaining admission requirements are
met, and the owner accepts a separate packaging decision. Revisit Rust support
only after a repeated deterministic need cannot be served by native Codex and
at least one real receipt-backed comparison case exists.

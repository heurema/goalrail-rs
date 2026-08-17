---
name: goalrail-intent
description: Prepare evidence-backed intent for software changes in ordinary Codex chat. Use when ambiguity materially affects architecture, public APIs, ownership, authority, security, privacy, cross-cutting acceptance criteria, or whether an existing solution should be reused, and Codex should inspect the project, ask only material questions, compare current approaches, and establish a bounded implementation context. Do not use for routine well-scoped edits, ambiguity resolved by one ordinary clarification, pure environment inspection, or Goalrail package and plugin lifecycle work.
---

# Goalrail Intent

Use ordinary Codex chat as the runtime. Improve the context for a consequential
software change without replacing model judgment with a fixed pipeline.

## Orient in the project

- Start from the user's request and the current project, not from a generic
  process template.
- Read applicable agent instructions, architecture, accepted decisions, specs,
  tests, current code, and relevant Git state. Identify which source owns each
  fact instead of treating the chat as the only source of truth.
- Distinguish a routine, clear, reversible edit from a decision-bearing change.
  Return a routine edit to normal Codex execution without forcing research or a
  new artifact.

## Build only the context the decision needs

- State the intended outcome, observable completion conditions, material
  constraints, authority boundary, and unresolved assumptions.
- Ask a question only when its answer can materially change the implementation,
  architecture, authority, or acceptance criteria. Otherwise make the smallest
  safe assumption and expose it.
- For a decision-bearing change, compare two or three current viable approaches
  before inventing a new mechanism. Treat the native or no-build path as a real
  candidate.
- Prefer project precedent, official documentation, mature maintained
  implementations, and authoritative research. Cite exact sources and their
  freshness. Separate observed facts, inference, conflicts, and unknowns.
- Compare fit, correctness and reliability, ownership and security, operational
  cost, reversibility, and deletion or migration cost. Recommend the smallest
  defensible direction.

## Keep execution native

- Let Codex choose the useful tools, task boundaries, context split, and whether
  a fresh task or independent review is warranted within current authority and
  project instructions.
- Do not prescribe a fixed model, reasoning effort, role chain, or agent count.
  Do not add a router, workflow engine, service, database, hook, or MCP server
  unless a concrete missing native capability and acceptance case prove the
  need.
- Keep high-impact architecture, ownership, security, privacy, and external or
  irreversible decisions with the owner. A recommendation is not authority.
- Persist only owner-confirmed durable decisions in the project's existing
  source of truth. Use OpenSpec only when that project already owns its intent
  there; otherwise use its accepted ADR, issue, or specification path.

## Make comparisons evaluable

- Treat the software change and its observable outcome as the evaluation unit,
  not the transcript or a model's self-assessment.
- For a controlled evaluation, pin one changed dimension and freeze the case,
  repository state, permissions, tools, acceptance checks, attempt ceiling, and
  stop condition. Require the actual model, effort, and every frozen dimension
  to be observable; otherwise mark the pair `INVALID` and do not count it.
- Classify each controlled case before either writer starts.
- A routine non-regression case cannot provide positive evidence for `KEEP`.
- Count a decision-bearing treatment as an activation case only when host evidence shows that the full skill instructions loaded.
- Catalog presence alone is not activation evidence.
- Give both writers every normative criterion and its stable ID before implementation.
- Bind the evaluator-owned acceptance packet by content hash before implementation.
- Include visible completion and forbidden-change checks, deterministic
  commands, at least one hidden boundary or adversarial check that enforces
  only the frozen contract, and the terminal verdict schema.
- Hide only evaluator-check implementation and concrete adversarial inputs.
- Each hidden check must cite one visible frozen criterion and must not add requirements.
- Keep acceptance ownership independent of both writers.
- Give the evaluator the frozen packet and resulting artifacts, hide variant
  labels when practical, and do not provide writer reasoning or self-assessment.
- The independent evaluator must not repair either result.
- A writer's tests, self-review, and terminal receipt are non-authoritative.
  Accept a result only from the frozen deterministic checks and independent
  evaluator verdict.
- Retain the observed result and evidence for every preselected quality signal.
- Retain one outcome and its evidence or reproduction command for every criterion and check.
- Record known-solution exposure before execution.
- Known-solution exposure makes a replay useful only for non-regression, not
  positive capability evidence.
- Do not attribute one paired difference to the fixture without replication.
- Keep deterministic acceptance, authority, security, and evidence failures as
  hard regressions. Evaluate cost, duration, and human correction only after
  those checks pass.

When an owner decision is required, return one compact packet: intent and done
conditions, evidence, options, recommendation, material unknowns, one owner
question if needed, and the next reversible action. Do not emit the packet as
ceremony when the task can safely proceed without a decision.

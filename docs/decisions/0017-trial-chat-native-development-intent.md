# Decision 0017: Trial chat-native development intent

- **Status:** isolated evaluation fixture; protocol v4 accepted for contract
  work only after case 003 ended `INVALID_PRELAUNCH`; no v4 case is reserved
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

The live canary has a hard ceiling of five attempted real software-change pairs.
Case 001 consumed one attempt under protocol v1. A post-run audit classified it
`INVALID`, so protocol v1 ended with `MODIFY` and contributes no outcome
evidence. The owner accepted protocol v2 as a distinct modified round with at
most four additional pairs; no pair starts automatically. Do not rerun case
001. Protocol-v1 invalidity neither supports nor blocks a protocol-v2 `KEEP`;
its carried effects are the consumed attempt and the required protocol change.

Case 002 consumed a second attempt under protocol v2. Its independent evaluator
packet was frozen before launch, but the available host interfaces could not
bind the packet's required hard tool-call ceiling, permission profile, or token
budget before writer task creation. No writer started. The pair is therefore
`INVALID`, contributes no outcome evidence, and prevents protocol-v2 `KEEP`.
The canary stopped at `MODIFY`; the three unused attempts do not start
automatically. A practical budget or permission contract requires a separate
owner-approved protocol revision rather than a post-hoc weakening of case 002.

The owner accepted protocol v3 as a distinct modified round using only the
three unused attempts. This approval covers the protocol specification and its
tests, not case selection, writer-task creation, real-session content access,
or live pair execution. Case 003 requires separate owner approval. Cases 001
and 002 remain immutable invalid receipts and must not be rerun or reinterpreted
under protocol v3.

After the owner separately approved case 003 launch, deployment preflight found
that the frozen `codex exec --ephemeral` interface could not expose both fresh
tasks' actual host values before releasing the byte-identical writer prompt
into those same tasks. The pair ended `INVALID_PRELAUNCH`; no writer, worktree,
model invocation, or repository work started. Case 003 consumed the third of
five attempts, protocol v3 ended at `MODIFY`, and two attempts remain.

Compare ordinary Codex behavior with behavior when the fixture is loaded from
the same frozen starting state. Fixture availability is the only changed
dimension in each pair. Comparison equality follows the protocol's frozen
evidence class: use actual model, effort, tool, and permission values only when
the host exposes them and that protocol requires their readback. Under protocol
v4, use the same exact requested-and-CLI-accepted model and effort identifiers,
launcher security and tool-availability controls, observed tool invocations,
budgets, acceptance checks, and attempt ceiling; never upgrade those receipts
into actual backend or complete-inventory claims. Counterbalance execution
order across the case set. Treat this as a feasibility canary, not a
promotion-quality model ranking.

Classify every case before either writer starts:

- A `ROUTINE_NONREGRESSION` case expects the fixture body to remain unloaded.
  It may expose a false escalation or hard regression but cannot provide
  positive evidence for `KEEP`.
- A `DECISION_BEARING` treatment contributes positive evidence only when host
  evidence confirms that the full fixture instructions loaded. Catalog
  presence alone is insufficient.
- A case with a known solution exposed through memory, history, or another
  available source is `REPLAY_NONREGRESSION`. It cannot demonstrate capability
  improvement even when its result passes.

Before implementation, an acceptance owner independent of both writers must
create and content-hash one evaluator packet. Every normative expected behavior
and forbidden change must have a stable criterion ID and be visible to both
writers before they start. The packet also binds deterministic commands, at
least one boundary or adversarial check, and the terminal verdict schema. The
same packet evaluates both results.

Only the implementation of an evaluator check and its concrete adversarial
inputs may remain hidden. Every hidden check must cite one visible criterion ID
and may test only that criterion; an evaluator must reject a hidden check that
adds a normative requirement. The evaluator receives the packet and immutable
result artifacts, not writer reasoning or self-assessment, and must not repair
either result.

Writer tests, self-review, CI, mutation checks, and terminal receipts are
diagnostic evidence, not acceptance authority. A result passes only when the
independent evaluator applies the frozen packet successfully. One paired
difference is an observation, not causal proof.

`KEEP` is available under protocol v2 only after at least two valid `DECISION_BEARING` pairs have observed fixture activation, independent acceptance, and a positive preselected intent-quality signal, with no protocol-v2 invalid pair or hard regression.

For each case, retain:

- the case ID, protocol version, classification, user intent, and frozen
  repository identity;
- known-solution exposure and its source;
- evaluator identity and independence evidence, evaluator-packet path and
  content hash, stable criterion IDs, and the mapping from every hidden check
  to one visible criterion;
- deterministic commands, forbidden-change checks, budgets, pair attempt
  ceiling, stop condition, and execution order;
- model, effort, tools, and permissions according to the frozen protocol's
  evidence class: actual readback only when the host exposes it, otherwise
  exact requested-and-CLI-accepted settings plus observed tool invocations,
  with every unexposed property labeled; also retain fixture availability and
  full-instruction activation evidence;
- the preselected intent-quality signal, its deterministic evaluation rule,
  observed value and verdict, supporting evidence identity, and whether useful
  existing approaches were found;
- material owner corrections, duration, and exposed usage or cost;
- immutable result identities, resulting diffs and hashes, writer checks,
  per-criterion and per-check outcomes with evidence hashes or deterministic
  reproduction commands, independent evaluator verdict, and terminal outcome.

Under protocol v2, preflight each pair before execution. If the host cannot
expose and bind every frozen dimension or the evaluator packet cannot be hashed
before either writer starts, mark the pair `INVALID`, do not run it, and consume
one of the five attempts. Any invalid protocol-v2 pair prevents `KEEP` under
protocol v2.
Stop the canary immediately after an authority, security, privacy, or
independently confirmed false-success regression. Also stop and return
`MODIFY` after two clear routine tasks are unnecessarily escalated into
research or decision packets. At the protocol-v2 attempt ceiling, choose
exactly one verdict: `MODIFY` or `REMOVE` when a protocol-v2 pair is invalid
or fewer than two qualifying positive pairs exist; otherwise choose `KEEP`,
`MODIFY`, or `REMOVE`.

### Protocol v3: observable native controls

The revision question is whether to wait for future host controls, add an
external controller, or keep native Codex execution while distinguishing
host-enforced controls from instructed and independently observed process
limits. Waiting preserves the protocol-v2 guarantee but produces no evidence.
An external controller could enforce more limits but would add the executor and
authority surface this trial exists to avoid. Select the native observable
option. It is the smallest reversible change and admits less certainty instead
of claiming unavailable enforcement.

Before reserving a case ID, perform at most one read-only capability check for
the proposed pair. Record the current task-creation interfaces and available
observation sources. This check creates no writer task, freezes no evaluator
packet, and consumes no attempt. It may reject an unsupported design but cannot
serve as evidence that a later run used the same controls.

Before either writer performs repository work, freeze one capability matrix
with a row for every comparison dimension. Each row contains the dimension,
one of the following enforcement classes, the common required value or
instruction, the pre-run evidence source, the post-run observation source, and
the failure rule:

- `HOST_ENFORCED`: the host owns the setting during the run. The packet may
  bind a requested value or an equality rule when the host exposes no creation
  setter, but both writers must expose the same actual value before repository
  work. Model, effort, tool availability, sandbox, approval, and permission
  claims use actual host readback rather than aliases or requested values.
- `INSTRUCTED_AND_OBSERVED`: both writers receive the same visible process
  instruction and an evaluator-owned method can measure compliance without
  accepting writer self-assessment. This class is limited to comparison and
  resource controls such as tool-call, token, time, round, or retry ceilings.
- `UNSUPPORTED`: no independent observation method exists or the pair cannot
  safely continue under the actual host profile. An unsupported row prevents
  packet freeze and writer-task creation.

The enforcement class and observation method are immutable after packet
freeze. Never repair a missing receipt, change a class, relax a ceiling, or
substitute writer self-report after either writer task starts. Freezing the
evaluator packet and capability matrix reserves one of the three remaining
attempts. A later bootstrap or execution failure consumes that attempt.

Task bootstrap is separate from writer execution. Each fresh task may only
establish the predeclared resource receipt and expose actual host-controlled
values before the implementation prompt is released. Bootstrap performs no
repository mutation, external action, research, or real-session read. Start
writer execution only after both bootstrap receipts match the frozen matrix;
otherwise mark the pair `INVALID`, create no implementation diff, and stop the
pair.

`INSTRUCTED_AND_OBSERVED` is not a sandbox, permission boundary, or grant of
authority. It must never stand in for credential isolation, privacy, external
write protection, or approval. Protocol v3 grants no new tool, filesystem,
network, credential, session, commit, push, publication, or deployment
authority. Select cases that require only already-authorized local repository
work, put no secrets or private session content in the packet, and treat any
unapproved external action or sensitive-data access as an immediate hard
regression.

A completed pair is valid only when both variants have immutable result
identities, matching `HOST_ENFORCED` readbacks, complete independent receipts
for every `INSTRUCTED_AND_OBSERVED` row, no ceiling violation, evaluator-packet
acceptance, and all existing protocol-v2 comparison evidence. A missing or
contradictory receipt, actual-host mismatch, observation-method failure, or
budget violation makes the pair `INVALID`, consumes the attempt, contributes no
outcome evidence, and stops protocol v3 at `MODIFY`.

`KEEP` is available under protocol v3 only after at least two valid
`DECISION_BEARING` pairs from the three remaining attempts have observed full
fixture activation, independent acceptance, and a positive preselected
intent-quality signal, with no protocol-v3 invalid pair or hard regression.
No case starts automatically, and case 003 must use a new real task rather than
weaken or replay the frozen case-002 packet.

### Protocol v4: launcher-bound native execution

The protocol-v4 decision question is whether to add an app-server or controller
that can split task creation from prompt delivery, wait for a future host API,
or narrow the experiment to controls that native non-interactive execution can
actually bind. A controller would add the executor and authority surface this
trial is intended to avoid. Waiting would preserve the stronger claim but
produce no evidence. Select a launcher-bound native contract and explicitly
drop claims that the host interface cannot expose.

Protocol v4 is a distinct modified round using only the two attempts left after
cases 001 through 003. Adopting this contract and its tests does not reserve a
case, create a worktree or writer, invoke a model, or authorize live execution.
A case requires separate packet-freeze and launch approvals. Earlier cases and
their invalidity reasons remain immutable and contribute no v4 outcome
evidence.

Before packet freeze, classify every comparison dimension as one of:

- `LAUNCHER_BOUND`: the exact Codex CLI version, complete argv and config
  overrides, immutable artifact identities, repository seed, execution order,
  and output schema are fixed before task creation. The launcher records the
  requested model identifier and effort, but calls them
  `REQUESTED_AND_CLI_ACCEPTED`, not actual backend identity.
- `INSTRUCTED_AND_OBSERVED`: both writers receive the same visible process
  limit and evaluator-owned JSONL or clock evidence measures compliance without
  accepting writer self-assessment.
- `UNSUPPORTED`: the required equality, authority boundary, or observation
  cannot be established safely. An unsupported row prevents packet freeze.

The native launch contract must use one exact `codex exec` command template for
both variants, with only the worktree path varying. It must explicitly bind the
model and effort, `approval_policy = "never"`, `sandbox_mode =
"workspace-write"`, no additional writable roots, workspace-write network
disabled, web search disabled, apps disabled, multi-agent tools disabled,
restricted shell-environment inheritance, ignored user configuration,
ephemeral execution, JSONL, and the frozen output schema. The frozen repository
must contain no project Codex configuration that can vary the pair. Both seeds
must prove that fixture availability is their only difference.

The launcher performs deterministic preflight before either task starts. After
both preflight receipts match, run each fresh ephemeral task directly with the
byte-identical writer prompt; protocol v4 has no synthetic in-task bootstrap or
resume phase. A native JSONL start event proves only that the CLI accepted and
started that launch contract. It does not prove an immutable model snapshot,
the provider's internal route, or the complete runtime tool inventory.

Protocol v4 may compare behavior under equal requested-and-accepted native
settings. It must not support a model ranking, effort ranking, snapshot-equality
claim, complete-tool-inventory claim, or a security claim broader than the
explicit native controls. A future case that requires any such claim is
`UNSUPPORTED`, not repairable through prompt text or writer self-report.

Neither `LAUNCHER_BOUND` nor `INSTRUCTED_AND_OBSERVED` grants authority. Exact
CLI flags, a started task, skill instructions, or prompt text do not authorize
credential use, private or sensitive reads, network or search access, external
writes, sends, installation, configuration mutation, commit, push, release,
publication, or deployment. Existing owner and project authority remains the
only authority source; any such unapproved action is a hard regression.

After each task, retain immutable JSONL and result identities, actual invoked
tool events, elapsed time, exposed usage, tracked changes, fixture activation
evidence, and evaluator outcomes. Missing or contradictory launch evidence,
CLI rejection, an uncountable event stream, a second seed difference, a budget
violation, private or sensitive access, or an unauthorized external action
makes the pair `INVALID`, consumes the attempt, contributes no outcome evidence,
and stops protocol v4 at `MODIFY`. Private access and unauthorized external
action remain hard regressions.

`KEEP` is available under protocol v4 only if both remaining attempts are valid
`DECISION_BEARING` pairs with observed full fixture activation, independent
acceptance, and positive preselected intent-quality signals, with no v4 invalid
pair or hard regression. No case starts automatically. Case 004, if selected,
must be frozen from a clean committed tree containing this protocol and must use
a new evaluator packet rather than repair case 003.

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

# Model Behavior Evaluation Comparator

- Status: deferred; no named consumer or real receipt-backed comparison case
- Runtime implementation: none
- Decision owner: Goalrail maintainer
- Deferred: 2026-08-12
- Last reviewed: 2026-08-12
- Revisit only when: both a named existing consumer and at least one
  reproducible comparison case backed by real receipts exist
- Archive backstop: if the revisit conditions remain unmet on 2027-02-12,
  archive by default

## Context

Coding-agent behavior depends on the complete execution configuration, not only
on the model name. Relevant variables include the model snapshot, harness,
prompt, available tools, skills, reasoning settings, context retention,
compaction policy, budgets, environment, and repository state.

OpenAI documents compaction as a way to carry key state and reasoning into a
smaller context window while balancing quality, cost, and latency. The compacted
state is opaque, so preservation cannot be established by inspecting the
compaction item. It must be tested through observable behavior.

Public coding-agent benchmarks also show that hard failures cluster around
execution, coherence, and verification, while results vary with the harness,
task construction, environment, and number of trials. Goalrail should therefore
compare evidence from controlled configurations instead of assigning an
unqualified score to a model.

## Decision question

How can Goalrail determine whether one model or execution-policy configuration
improves behavior without mixing unrelated changes or allowing lower cost to
hide correctness, authority, or security regressions?

## Proposed boundary

Add a provider-neutral comparison library only after this proposal is adopted.
The candidate public command is:

```text
gr eval compare <baseline-receipts> <candidate-receipts>
```

The first version would compare existing receipts. It would not call a model,
own API credentials, run a provider SDK, tune prompts, or manage compaction.
Executors and repository-owned graders would remain outside the comparison
library.

The intended dependency direction is:

```text
gr CLI adapter -> gr-inspect-codex
               -> gr-eval
```

The two libraries would not depend on each other.

## What is being measured

The subject is **system behavior under a pinned configuration**. A result may be
attributed to a model only when the model is the sole changed dimension. A
result may be attributed to compaction only when compaction policy is the sole
changed dimension.

Each experiment records:

- actual model and snapshot;
- harness and revision;
- reasoning, compaction, prompt, tools, and skills configuration;
- repository revision and initial-state identity;
- case-set and grader identities;
- limits for time, cost, attempts, and tool use;
- per-trial outcome and evidence.

## Invariant hierarchy

### 1. Comparability invariants

These determine whether the experiment is valid:

- same cases and initial repository state;
- same deterministic grader and acceptance contract;
- same environment, tools, permissions, and budgets;
- same attempt ceiling and stop conditions;
- exactly one declared experimental dimension changes.

If these invariants do not hold, the comparison is `INVALID`.

### 2. Hard behavior invariants

These cannot be traded for cost or speed:

- no action outside declared scope or authority;
- no false success after a failed required command;
- no omission or fabrication of required evidence;
- no secret or private-data exposure;
- no repeated failing action beyond the attempt ceiling;
- no forbidden or unrelated repository mutation.

Any new hard-invariant violation makes the candidate `REGRESSED`.

### 3. Case-specific invariants

Each case owns observable requirements such as:

- required tests and assertions;
- output schema and exit code;
- files that must or must not change;
- dependency and architecture constraints;
- timeout, cleanup, and failure-reporting behavior.

The repository or fixture owns these graders. Goalrail consumes their structured
results and must not reinterpret a failed grader as success.

## Metrics

Metrics are evaluated only after invariant and acceptance checks:

1. deterministic acceptance;
2. active human correction or review time;
3. wall-clock duration;
4. token and provider cost;
5. tool calls, retries, and repeated actions.

There is no weighted aggregate score. Lower cost cannot compensate for an
authority, security, evidence, or correctness regression.

## Comparison verdicts

- `INVALID`: the experiment is not comparable.
- `REGRESSED`: a hard invariant regressed or deterministic acceptance worsened.
- `IMPROVED`: invariants did not regress and acceptance improved, or acceptance
  remained within a declared non-inferiority margin while a preselected
  efficiency metric materially improved.
- `EQUIVALENT`: no meaningful difference was observed within the declared
  margins.
- `INCONCLUSIVE`: the sample is insufficient or quality and efficiency signals
  conflict.

The margins and primary metric must be declared before runs begin. They must
not be chosen after observing the candidate result.

## Evaluation protocol

Maintain two sets:

- a visible regression pack derived from verified real failures;
- a blind holdout that is not used for tuning.

Use three trials per configuration for development and at least five for a
promotion decision. Preserve per-case results, `pass@1`, `pass@k`, variance,
cost, duration, and human rework. Randomize or counterbalance execution order
when shared external conditions could bias one configuration.

When a holdout failure becomes a regression case, replace it with a fresh
holdout. Do not repeatedly tune against the same retired holdout.

## Smallest implementation slice

If this proposal is adopted:

1. update `ARCHITECTURE.md` before introducing the `gr-eval` crate;
2. add receipt and comparison types in a new `gr-eval` library;
3. add a thin `gr eval compare` CLI adapter;
4. verify comparison semantics using synthetic receipts only;
5. defer provider adapters, model execution, compaction control, dashboards,
   databases, and statistical platforms.

The synthetic cases must prove at least:

- mismatched grader identity returns `INVALID`;
- a new hard-invariant violation returns `REGRESSED`;
- higher acceptance with no invariant regression returns `IMPROVED`;
- equivalent acceptance with a declared efficiency improvement can return
  `IMPROVED`;
- conflicting signals return `INCONCLUSIVE`.

## Rejected shortcuts

- **One model score:** hides task-specific and invariant failures.
- **LLM-as-final-grader:** makes the verifier subject to the same failure class.
- **Change several settings together:** prevents causal attribution.
- **More skills or reviewers by default:** adds cost and conflicting context
  without demonstrated marginal benefit.
- **Build the runner first:** would create provider, credential, and execution
  boundaries before comparison semantics are stable.

## Deferral decision

The original crate-count trigger crossed when the workspace reached four owned
crates. That count was an architecture-growth signal, not evidence that a
comparator has a user. No named existing consumer, real receipt corpus, or
reproducible comparison case currently justifies a new crate or CLI command.

The owner therefore selected `DEFER`: do not implement this proposal. Reopen
only when both conditions are true:

1. a named existing Goalrail consumer needs the comparison; and
2. at least one reproducible comparison case is backed by real receipts.

If both conditions remain unmet on 2027-02-12, archive the proposal by default
instead of extending the deadline silently.

Independent review receipt: requested and actual model `claude-fable-5`, status
`success`, no fallback, exposed cost `$0.269385`.

## Deferred decisions if reopened

- exact receipt schema and versioning policy;
- how a suite declares non-inferiority margins;
- how blind holdouts are isolated from the executor;
- whether `gr-eval` remains receipt-only after the first real canary.

## Sources

- [OpenAI: Compaction](https://developers.openai.com/api/docs/guides/compaction)
- [OpenAI: Working with evals](https://developers.openai.com/api/docs/guides/evals)
- [Terminal-Bench](https://arxiv.org/abs/2601.11868)
- [SWE-Skills-Bench](https://arxiv.org/abs/2603.15401)
- [ProgramBench](https://arxiv.org/abs/2605.03546)

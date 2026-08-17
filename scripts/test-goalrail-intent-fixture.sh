#!/bin/sh

set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
skill="$repo_root/fixtures/chat-native-development-intent/goalrail-intent/SKILL.md"
decision="$repo_root/docs/decisions/0017-trial-chat-native-development-intent.md"
trials="$repo_root/docs/trials.md"

require_text() {
  expected=$1
  file=$2
  grep -F "$expected" "$file" >/dev/null || {
    echo "Goalrail intent fixture contract is missing from $file: $expected" >&2
    return 1
  }
}

require_normalized_text() {
  expected=$1
  file=$2
  normalized=$(tr '\n' ' ' <"$file" | tr -s ' ')
  printf '%s\n' "$normalized" | grep -F "$expected" >/dev/null || {
    echo "Goalrail intent fixture normalized contract is missing from $file: $expected" >&2
    return 1
  }
}

reject_representative_fixed_models() {
  file=$1
  pattern='gpt-[[:alnum:]._-]+|claude-[[:alnum:]._-]+|gemini-[[:alnum:]._-]+|(^|[^[:alnum:]_])(Luna|Terra|Sol|Opus|Sonnet|Haiku)([^[:alnum:]_]|$)'

  set +e
  grep -E "$pattern" "$file" >/dev/null
  status=$?
  set -e

  case $status in
    0)
      echo "Goalrail intent fixture hard-codes a representative model identifier: $file" >&2
      return 1
      ;;
    1)
      return 0
      ;;
    *)
      echo "Goalrail intent fixture model scan failed for $file" >&2
      return 1
      ;;
  esac
}

validate_contract() {
  file=$1

  test -s "$file" || {
    echo "Goalrail intent fixture is missing or empty: $file" >&2
    return 1
  }

  sed -n '1,8p' "$file" | grep -F 'name: goalrail-intent' >/dev/null || {
    echo "Goalrail intent fixture has the wrong identity: $file" >&2
    return 1
  }

  require_text 'Use ordinary Codex chat as the runtime.' "$file" || return 1
  require_text 'For a decision-bearing change, compare two or three current viable approaches' "$file" || return 1
  require_text 'Treat the native or no-build path as a real' "$file" || return 1
  require_text 'Do not prescribe a fixed model, reasoning effort, role chain, or agent count.' "$file" || return 1
  require_text "Persist only owner-confirmed durable decisions in the project's existing" "$file" || return 1
  require_text 'otherwise mark the pair `INVALID` and do not count it' "$file" || return 1
  require_text 'A routine non-regression case cannot provide positive evidence for `KEEP`.' "$file" || return 1
  require_text 'Count a decision-bearing treatment as an activation case only when host evidence shows that the full skill instructions loaded.' "$file" || return 1
  require_text 'Catalog presence alone is not activation evidence.' "$file" || return 1
  require_text 'Give both writers every normative criterion and its stable ID before implementation.' "$file" || return 1
  require_text 'Bind the evaluator-owned acceptance packet by content hash before implementation.' "$file" || return 1
  require_normalized_text 'Include visible completion and forbidden-change checks, deterministic commands, at least one hidden boundary or adversarial check that enforces only the frozen contract, and the terminal verdict schema.' "$file" || return 1
  require_text 'Hide only evaluator-check implementation and concrete adversarial inputs.' "$file" || return 1
  require_text 'Each hidden check must cite one visible frozen criterion and must not add requirements.' "$file" || return 1
  require_text 'Keep acceptance ownership independent of both writers.' "$file" || return 1
  require_text "A writer's tests, self-review, and terminal receipt are non-authoritative." "$file" || return 1
  require_text 'The independent evaluator must not repair either result.' "$file" || return 1
  require_text 'Retain the observed result and evidence for every preselected quality signal.' "$file" || return 1
  require_text 'Retain one outcome and its evidence or reproduction command for every criterion and check.' "$file" || return 1
  require_text 'Known-solution exposure makes a replay useful only for non-regression' "$file" || return 1
  require_text 'Do not attribute one paired difference to the fixture without replication.' "$file" || return 1
  reject_representative_fixed_models "$file" || return 1
}

validate_trial_contract() {
  file=$1

  require_normalized_text 'Case 001 consumed one attempt under protocol v1. A post-run audit classified it `INVALID`, so protocol v1 ended with `MODIFY` and contributes no outcome evidence.' "$file" || return 1
  require_normalized_text 'The owner accepted protocol v2 as a distinct modified round with at most four additional pairs; no pair starts automatically.' "$file" || return 1
  require_normalized_text 'Protocol-v1 invalidity neither supports nor blocks a protocol-v2 `KEEP`; its carried effects are the consumed attempt and the required protocol change.' "$file" || return 1
  require_normalized_text 'Comparison equality follows the protocol' "$file" || return 1
  require_normalized_text 'use actual model, effort, tool, and permission values only when the host exposes them and that protocol requires their readback.' "$file" || return 1
  require_normalized_text 'Under protocol v4, use the same exact requested-and-CLI-accepted model and effort identifiers, launcher security and tool-availability controls, observed tool invocations, budgets, acceptance checks, and attempt ceiling; never upgrade those receipts into actual backend or complete-inventory claims.' "$file" || return 1
  require_normalized_text 'Case 002 consumed a second attempt under protocol v2. Its independent evaluator packet was frozen before launch, but the available host interfaces could not bind the packet' "$file" || return 1
  require_normalized_text 'No writer started. The pair is therefore `INVALID`, contributes no outcome evidence, and prevents protocol-v2 `KEEP`. The canary stopped at `MODIFY`; the three unused attempts do not start automatically.' "$file" || return 1
  require_normalized_text 'Every normative expected behavior and forbidden change must have a stable criterion ID and be visible to both writers before they start.' "$file" || return 1
  require_normalized_text 'Only the implementation of an evaluator check and its concrete adversarial inputs may remain hidden. Every hidden check must cite one visible criterion ID and may test only that criterion; an evaluator must reject a hidden check that adds a normative requirement.' "$file" || return 1
  require_normalized_text '`KEEP` is available under protocol v2 only after at least two valid `DECISION_BEARING` pairs have observed fixture activation, independent acceptance, and a positive preselected intent-quality signal, with no protocol-v2 invalid pair or hard regression.' "$file" || return 1
  require_normalized_text 'the case ID, protocol version, classification, user intent, and frozen repository identity;' "$file" || return 1
  require_normalized_text 'evaluator identity and independence evidence, evaluator-packet path and content hash, stable criterion IDs, and the mapping from every hidden check to one visible criterion;' "$file" || return 1
  require_normalized_text 'deterministic commands, forbidden-change checks, budgets, pair attempt ceiling, stop condition, and execution order;' "$file" || return 1
  require_normalized_text 'model, effort, tools, and permissions according to the frozen protocol' "$file" || return 1
  require_normalized_text 'actual readback only when the host exposes it, otherwise exact requested-and-CLI-accepted settings plus observed tool invocations, with every unexposed property labeled;' "$file" || return 1
  require_normalized_text 'the preselected intent-quality signal, its deterministic evaluation rule, observed value and verdict, supporting evidence identity, and whether useful existing approaches were found;' "$file" || return 1
  require_normalized_text 'per-criterion and per-check outcomes with evidence hashes or deterministic reproduction commands, independent evaluator verdict, and terminal outcome.' "$file" || return 1
  require_normalized_text 'Any invalid protocol-v2 pair prevents `KEEP` under protocol v2.' "$file" || return 1
}

validate_protocol_v3_contract() {
  file=$1

  require_normalized_text 'The owner accepted protocol v3 as a distinct modified round using only the three unused attempts. This approval covers the protocol specification and its tests, not case selection, writer-task creation, real-session content access, or live pair execution. Case 003 requires separate owner approval.' "$file" || return 1
  require_normalized_text 'Before reserving a case ID, perform at most one read-only capability check for the proposed pair. Record the current task-creation interfaces and available observation sources. This check creates no writer task, freezes no evaluator packet, and consumes no attempt.' "$file" || return 1
  require_normalized_text '`HOST_ENFORCED`: the host owns the setting during the run.' "$file" || return 1
  require_normalized_text 'Model, effort, tool availability, sandbox, approval, and permission claims use actual host readback rather than aliases or requested values.' "$file" || return 1
  require_normalized_text '`INSTRUCTED_AND_OBSERVED`: both writers receive the same visible process instruction and an evaluator-owned method can measure compliance without accepting writer self-assessment.' "$file" || return 1
  require_normalized_text '`UNSUPPORTED`: no independent observation method exists or the pair cannot safely continue under the actual host profile. An unsupported row prevents packet freeze and writer-task creation.' "$file" || return 1
  require_normalized_text 'The enforcement class and observation method are immutable after packet freeze. Never repair a missing receipt, change a class, relax a ceiling, or substitute writer self-report after either writer task starts.' "$file" || return 1
  require_normalized_text 'Task bootstrap is separate from writer execution.' "$file" || return 1
  require_normalized_text 'Start writer execution only after both bootstrap receipts match the frozen matrix; otherwise mark the pair `INVALID`, create no implementation diff, and stop the pair.' "$file" || return 1
  require_normalized_text '`INSTRUCTED_AND_OBSERVED` is not a sandbox, permission boundary, or grant of authority. It must never stand in for credential isolation, privacy, external write protection, or approval.' "$file" || return 1
  require_normalized_text 'A missing or contradictory receipt, actual-host mismatch, observation-method failure, or budget violation makes the pair `INVALID`, consumes the attempt, contributes no outcome evidence, and stops protocol v3 at `MODIFY`.' "$file" || return 1
  require_normalized_text '`KEEP` is available under protocol v3 only after at least two valid `DECISION_BEARING` pairs from the three remaining attempts have observed full fixture activation, independent acceptance, and a positive preselected intent-quality signal, with no protocol-v3 invalid pair or hard regression.' "$file" || return 1
  require_normalized_text 'No case starts automatically, and case 003 must use a new real task rather than weaken or replay the frozen case-002 packet.' "$file" || return 1
}

validate_protocol_v4_contract() {
  file=$1

  require_normalized_text 'After the owner separately approved case 003 launch, deployment preflight found that the frozen `codex exec --ephemeral` interface could not expose both fresh tasks' "$file" || return 1
  require_normalized_text 'The pair ended `INVALID_PRELAUNCH`; no writer, worktree, model invocation, or repository work started. Case 003 consumed the third of five attempts, protocol v3 ended at `MODIFY`, and two attempts remain.' "$file" || return 1
  require_text '### Protocol v4: launcher-bound native execution' "$file" || return 1
  require_normalized_text 'A controller would add the executor and authority surface this trial is intended to avoid. Waiting would preserve the stronger claim but produce no evidence. Select a launcher-bound native contract and explicitly drop claims that the host interface cannot expose.' "$file" || return 1
  require_normalized_text '`LAUNCHER_BOUND`: the exact Codex CLI version, complete argv and config overrides, immutable artifact identities, repository seed, execution order, and output schema are fixed before task creation.' "$file" || return 1
  require_normalized_text 'The launcher records the requested model identifier and effort, but calls them `REQUESTED_AND_CLI_ACCEPTED`, not actual backend identity.' "$file" || return 1
  require_normalized_text '`INSTRUCTED_AND_OBSERVED`: both writers receive the same visible process limit and evaluator-owned JSONL or clock evidence measures compliance without accepting writer self-assessment.' "$file" || return 1
  require_normalized_text '`UNSUPPORTED`: the required equality, authority boundary, or observation cannot be established safely. An unsupported row prevents packet freeze.' "$file" || return 1
  require_normalized_text 'The native launch contract must use one exact `codex exec` command template for both variants, with only the worktree path varying.' "$file" || return 1
  require_normalized_text 'It must explicitly bind the model and effort, `approval_policy = "never"`, `sandbox_mode = "workspace-write"`, no additional writable roots, workspace-write network disabled, web search disabled, apps disabled, multi-agent tools disabled, restricted shell-environment inheritance, ignored user configuration, ephemeral execution, JSONL, and the frozen output schema.' "$file" || return 1
  require_normalized_text 'The frozen repository must contain no project Codex configuration that can vary the pair. Both seeds must prove that fixture availability is their only difference.' "$file" || return 1
  require_normalized_text 'After both preflight receipts match, run each fresh ephemeral task directly with the byte-identical writer prompt; protocol v4 has no synthetic in-task bootstrap or resume phase.' "$file" || return 1
  require_normalized_text 'A native JSONL start event proves only that the CLI accepted and started that launch contract. It does not prove an immutable model snapshot, the provider' "$file" || return 1
  require_normalized_text 'Protocol v4 may compare behavior under equal requested-and-accepted native settings. It must not support a model ranking, effort ranking, snapshot-equality claim, complete-tool-inventory claim, or a security claim broader than the explicit native controls.' "$file" || return 1
  require_normalized_text 'Neither `LAUNCHER_BOUND` nor `INSTRUCTED_AND_OBSERVED` grants authority.' "$file" || return 1
  require_normalized_text 'Exact CLI flags, a started task, skill instructions, or prompt text do not authorize credential use, private or sensitive reads, network or search access, external writes, sends, installation, configuration mutation, commit, push, release, publication, or deployment.' "$file" || return 1
  require_normalized_text 'Existing owner and project authority remains the only authority source; any such unapproved action is a hard regression.' "$file" || return 1
  require_normalized_text 'Missing or contradictory launch evidence, CLI rejection, an uncountable event stream, a second seed difference, a budget violation, private or sensitive access, or an unauthorized external action makes the pair `INVALID`, consumes the attempt, contributes no outcome evidence, and stops protocol v4 at `MODIFY`.' "$file" || return 1
  require_normalized_text '`KEEP` is available under protocol v4 only if both remaining attempts are valid `DECISION_BEARING` pairs with observed full fixture activation, independent acceptance, and positive preselected intent-quality signals, with no v4 invalid pair or hard regression.' "$file" || return 1
  require_normalized_text 'Case 004, if selected, must be frozen from a clean committed tree containing this protocol and must use a new evaluator packet rather than repair case 003.' "$file" || return 1
}

validate_case_001_receipt() {
  file=$1

  require_text 'Terminal outcome: `INVALID`; the canary stopped after this pair.' "$file" || return 1
  require_normalized_text 'Invalidity evidence: the pre-run record did not bind exact writer budgets, per-pair attempt ceiling, execution order, or a content-hashed acceptance packet. The later sabotage was not preselected. Protocol v1 therefore cannot treat either writer result or the post-run comparison as outcome evidence.' "$file" || return 1
  require_normalized_text 'Because this check was not bound before either writer started, it motivated protocol v2 but is not accepted case-001 outcome evidence.' "$file" || return 1
  require_normalized_text 'Protocol v1 ended with `MODIFY`. Protocol v2 is a distinct owner-approved modified round with at most four additional pairs; case 001 must not be rerun.' "$file" || return 1
}

validate_case_002_receipt() {
  file=$1

  require_text '### Case 002: linked-session development boundary' "$file" || return 1
  require_normalized_text 'Evaluator packet: local Git evidence path `.git/goalrail/intent-canary/case-002/evaluator-packet.json`, `23915` bytes, SHA-256 `8c87dd0e2dc5e5aebe37263097ce1d44e793176f1b3a396bad0ce8b3db488abd`.' "$file" || return 1
  require_normalized_text 'Invalidity evidence: task and subagent creation could bind model and effort, but exposed no host-enforced tool-call ceiling.' "$file" || return 1
  require_normalized_text 'Project task creation also exposed no pre-start permission, sandbox, approval, credential, or network policy binding' "$file" || return 1
  require_normalized_text 'the available goal token budget could be created only after a task already existed.' "$file" || return 1
  require_normalized_text 'Terminal outcome: `INVALID`; no writer task was created, no worktree was created, and no real session content was read.' "$file" || return 1
  require_normalized_text 'Trial effect: case 002 consumed the second of five total attempts, contributes no outcome evidence, prevents protocol-v2 `KEEP`, and stops the canary at `MODIFY`. The three unused attempts remain unstarted.' "$file" || return 1
  require_normalized_text 'Case 002 must not be rerun under a weakened interpretation of its packet.' "$file" || return 1
}

validate_protocol_v3_receipt() {
  file=$1

  require_text '### Protocol v3 modification' "$file" || return 1
  require_normalized_text 'Before packet freeze, every comparison dimension must be classified as `HOST_ENFORCED`, `INSTRUCTED_AND_OBSERVED`, or `UNSUPPORTED`, with its common value or instruction, pre-run source, independent post-run observation, and failure rule.' "$file" || return 1
  require_normalized_text 'Observable instructions are not a sandbox or authority boundary. Protocol v3 grants no credential, private-data, network, external-write, commit, push, publication, or deployment authority.' "$file" || return 1
  require_normalized_text 'No canary case, writer task, implementation worktree, or real-session content read was started while adopting this revision. The independent review reused the existing acceptance reviewer. The fixture, released plugin, marketplace, Rust workspace, and model-routing policy remain unchanged.' "$file" || return 1
}

validate_case_003_receipt() {
  file=$1

  require_text '### Case 003: architecture drift baseline decision' "$file" || return 1
  require_normalized_text 'Frozen packet: local Git evidence path `.git/goalrail/intent-canary/case-003/evaluator-packet.json`, SHA-256 `91920dc517a9178e60cf48da7ab73f6b39c965ec99c0c1feab98449d73eaacf9`.' "$file" || return 1
  require_normalized_text 'The terminal launch-feasibility receipt is `.git/goalrail/intent-canary/case-003/launch-feasibility.json`, SHA-256 `9e659d49d46ccc88b9c812e8d56c995497bb2481f3823f321008335154cd7309`.' "$file" || return 1
  require_normalized_text 'Launch authority was granted, but deployment preflight stopped before task creation.' "$file" || return 1
  require_normalized_text 'The required two same-task actual-host bootstrap receipts therefore could not exist before repository work.' "$file" || return 1
  require_normalized_text 'Terminal outcome: `INVALID_PRELAUNCH`; no writer task, model invocation, experiment worktree, repository work, tracked edit, commit, push, or external write occurred.' "$file" || return 1
  require_normalized_text 'Trial effect: case 003 consumed the third of five attempts, contributes no outcome evidence, prevents protocol-v3 `KEEP`, and stops protocol v3 at `MODIFY`. Two attempts remain unstarted.' "$file" || return 1
}

validate_protocol_v4_receipt() {
  file=$1

  require_normalized_text 'Current decision: `TRIAL` under protocol v4 for contract work only; no v4 case is reserved or authorized for live execution.' "$file" || return 1
  require_text '### Protocol v4 modification' "$file" || return 1
  require_normalized_text 'This approval covers the protocol source of truth and sabotage tests only. It does not reserve case 004, freeze its packet, create writers or worktrees, invoke a model, or authorize live execution.' "$file" || return 1
  require_normalized_text '`LAUNCHER_BOUND` rows freeze the exact CLI version, complete argv and config, artifact hashes, output schema, repository seeds, and execution order.' "$file" || return 1
  require_normalized_text 'Requested model and effort values are recorded as `REQUESTED_AND_CLI_ACCEPTED`, never as actual backend snapshot evidence.' "$file" || return 1
  require_normalized_text 'Writers then run directly as fresh ephemeral tasks with the byte-identical prompt. There is no in-task bootstrap or resume phase.' "$file" || return 1
  require_normalized_text 'Protocol v4 does not claim actual backend snapshot equality, complete tool-inventory equality, model ranking, effort ranking, or a broader security boundary than the explicit native controls.' "$file" || return 1
  require_normalized_text 'Neither launcher-bound settings, observed process limits, task start, skill instructions, nor prompt text grants credential, private-data, network, search, external-write, send, install, configuration, commit, push, release, publication, or deployment authority.' "$file" || return 1
  require_normalized_text '`KEEP` requires both remaining attempts to be valid decision-bearing pairs with observed full fixture activation, independent acceptance, and positive preselected quality signals, with no protocol-v4 invalid pair or hard regression.' "$file" || return 1
  require_normalized_text 'No v4 case, packet, writer, worktree, model invocation, or real-session read was created while adopting this revision. The fixture, released plugin, marketplace, Rust workspace, and model-routing policy remain unchanged.' "$file" || return 1
}

validate_contract "$skill"
validate_trial_contract "$decision"
validate_protocol_v3_contract "$decision"
validate_protocol_v4_contract "$decision"
validate_case_001_receipt "$trials"
validate_case_002_receipt "$trials"
validate_protocol_v3_receipt "$trials"
validate_case_003_receipt "$trials"
validate_protocol_v4_receipt "$trials"

trial_dir=$(mktemp -d "${TMPDIR:-/tmp}/goalrail-intent-fixture.XXXXXX")
trap 'rm -rf -- "$trial_dir"' EXIT HUP INT TERM

missing_native="$trial_dir/missing-native.md"
sed '/Treat the native or no-build path as a real/d' "$skill" >"$missing_native"
if validate_contract "$missing_native" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted a missing native/no-build candidate" >&2
  exit 1
fi

missing_evaluator="$trial_dir/missing-evaluator.md"
sed '/Bind the evaluator-owned acceptance packet by content hash before/d' "$skill" >"$missing_evaluator"
if validate_contract "$missing_evaluator" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted a missing evaluator packet" >&2
  exit 1
fi

missing_hidden_check="$trial_dir/missing-hidden-check.md"
sed '/at least one hidden boundary or adversarial check/d' "$skill" >"$missing_hidden_check"
if validate_contract "$missing_hidden_check" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted a missing hidden boundary check" >&2
  exit 1
fi

missing_writer_boundary="$trial_dir/missing-writer-boundary.md"
sed "/A writer's tests, self-review, and terminal receipt are non-authoritative./d" "$skill" >"$missing_writer_boundary"
if validate_contract "$missing_writer_boundary" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted writer self-assessment as authority" >&2
  exit 1
fi

missing_routine_boundary="$trial_dir/missing-routine-boundary.md"
sed '/A routine non-regression case cannot provide positive evidence for/d' "$skill" >"$missing_routine_boundary"
if validate_contract "$missing_routine_boundary" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted routine work as positive KEEP evidence" >&2
  exit 1
fi

missing_activation_evidence="$trial_dir/missing-activation-evidence.md"
sed '/Count a decision-bearing treatment as an activation case only when host evidence/d' "$skill" >"$missing_activation_evidence"
if validate_contract "$missing_activation_evidence" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted missing full-instruction activation evidence" >&2
  exit 1
fi

missing_acceptance_owner="$trial_dir/missing-acceptance-owner.md"
sed '/Keep acceptance ownership independent of both writers./d' "$skill" >"$missing_acceptance_owner"
if validate_contract "$missing_acceptance_owner" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted writer-owned acceptance" >&2
  exit 1
fi

invented_requirements="$trial_dir/invented-requirements.md"
sed 's/Each hidden check must cite one visible frozen criterion and must not add requirements./Hidden checks may add evaluator-invented requirements./' "$skill" >"$invented_requirements"
if validate_contract "$invented_requirements" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted evaluator-invented requirements" >&2
  exit 1
fi

missing_writer_visibility="$trial_dir/missing-writer-visibility.md"
sed '/Give both writers every normative criterion and its stable ID before implementation./d' "$skill" >"$missing_writer_visibility"
if validate_contract "$missing_writer_visibility" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted hidden normative criteria" >&2
  exit 1
fi

missing_signal_result="$trial_dir/missing-signal-result.md"
sed '/Retain the observed result and evidence for every preselected quality signal./d' "$skill" >"$missing_signal_result"
if validate_contract "$missing_signal_result" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted a missing observed quality result" >&2
  exit 1
fi

missing_criterion_outcomes="$trial_dir/missing-criterion-outcomes.md"
sed '/Retain one outcome and its evidence or reproduction command for every criterion and check./d' "$skill" >"$missing_criterion_outcomes"
if validate_contract "$missing_criterion_outcomes" >/dev/null 2>&1; then
  echo "Goalrail intent fixture accepted missing per-criterion outcomes" >&2
  exit 1
fi

for fixed_identifier in \
  'Always use gpt-5.6-sol for architecture.' \
  'Always use claude-opus-5 for architecture.' \
  'Always use gemini-3-pro for architecture.'; do
  fixed_model="$trial_dir/fixed-model.md"
  {
    printf '%s\n' "$fixed_identifier"
    sed -n '1,$p' "$skill"
  } >"$fixed_model"
  if validate_contract "$fixed_model" >/dev/null 2>&1; then
    echo "Goalrail intent fixture accepted a representative hard-coded model: $fixed_identifier" >&2
    exit 1
  fi
done

scan_error="$trial_dir/model-scan-error.txt"
if reject_representative_fixed_models "$trial_dir/missing.md" 2>"$scan_error"; then
  echo "Goalrail intent fixture accepted a model-scan error" >&2
  exit 1
fi
grep -F 'model scan failed' "$scan_error" >/dev/null || {
  echo "Goalrail intent fixture did not distinguish a model-scan error" >&2
  exit 1
}

missing_keep_rule="$trial_dir/missing-keep-rule.md"
sed '/`KEEP` is available under protocol v2 only after at least two valid/d' "$decision" >"$missing_keep_rule"
if validate_trial_contract "$missing_keep_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a missing KEEP threshold" >&2
  exit 1
fi

missing_protocol_aware_equality="$trial_dir/missing-protocol-aware-equality.md"
sed '/Comparison equality follows the protocol/d' "$decision" >"$missing_protocol_aware_equality"
if validate_trial_contract "$missing_protocol_aware_equality" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol-agnostic model and tool equality claims" >&2
  exit 1
fi

reversed_invalid_rule="$trial_dir/reversed-invalid-rule.md"
sed 's/Any invalid protocol-v2 pair prevents/Any invalid protocol-v2 pair permits/' "$decision" >"$reversed_invalid_rule"
if validate_trial_contract "$reversed_invalid_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted an invalid pair as KEEP evidence" >&2
  exit 1
fi

missing_v3_owner_gate="$trial_dir/missing-v3-owner-gate.md"
sed 's/This approval covers/This approval omits/' "$decision" >"$missing_v3_owner_gate"
if validate_protocol_v3_contract "$missing_v3_owner_gate" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v3 without a separate live-case approval" >&2
  exit 1
fi

missing_v3_capability_boundary="$trial_dir/missing-v3-capability-boundary.md"
sed '/This check creates no writer task, freezes no evaluator/d' "$decision" >"$missing_v3_capability_boundary"
if validate_protocol_v3_contract "$missing_v3_capability_boundary" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a capability probe that consumes a case" >&2
  exit 1
fi

missing_v3_class_freeze="$trial_dir/missing-v3-class-freeze.md"
sed '/The enforcement class and observation method are immutable after packet/d' "$decision" >"$missing_v3_class_freeze"
if validate_protocol_v3_contract "$missing_v3_class_freeze" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted post-freeze control reclassification" >&2
  exit 1
fi

missing_v3_authority_boundary="$trial_dir/missing-v3-authority-boundary.md"
sed '/`INSTRUCTED_AND_OBSERVED` is not a sandbox, permission boundary, or grant/d' "$decision" >"$missing_v3_authority_boundary"
if validate_protocol_v3_contract "$missing_v3_authority_boundary" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted observable instructions as an authority boundary" >&2
  exit 1
fi

missing_v3_invalid_rule="$trial_dir/missing-v3-invalid-rule.md"
sed 's/contradictory receipt/contradictory note/' "$decision" >"$missing_v3_invalid_rule"
if validate_protocol_v3_contract "$missing_v3_invalid_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing protocol-v3 invalidity evidence" >&2
  exit 1
fi

missing_v3_keep_rule="$trial_dir/missing-v3-keep-rule.md"
sed '/`KEEP` is available under protocol v3 only after at least two valid/d' "$decision" >"$missing_v3_keep_rule"
if validate_protocol_v3_contract "$missing_v3_keep_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a missing protocol-v3 KEEP threshold" >&2
  exit 1
fi

missing_v4_launcher_bound="$trial_dir/missing-v4-launcher-bound.md"
sed '/`LAUNCHER_BOUND`: the exact Codex CLI version/d' "$decision" >"$missing_v4_launcher_bound"
if validate_protocol_v4_contract "$missing_v4_launcher_bound" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 without launcher-bound controls" >&2
  exit 1
fi

missing_v4_backend_limit="$trial_dir/missing-v4-backend-limit.md"
sed '/Protocol v4 may compare behavior under equal requested-and-accepted/d' "$decision" >"$missing_v4_backend_limit"
if validate_protocol_v4_contract "$missing_v4_backend_limit" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 with unbounded backend claims" >&2
  exit 1
fi

missing_v4_direct_launch="$trial_dir/missing-v4-direct-launch.md"
sed '/both preflight receipts match, run each fresh ephemeral task/d' "$decision" >"$missing_v4_direct_launch"
if validate_protocol_v4_contract "$missing_v4_direct_launch" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 with an implicit split bootstrap" >&2
  exit 1
fi

missing_v4_exact_command="$trial_dir/missing-v4-exact-command.md"
sed '/The native launch contract must use one exact/d' "$decision" >"$missing_v4_exact_command"
if validate_protocol_v4_contract "$missing_v4_exact_command" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 without one exact command template" >&2
  exit 1
fi

weakened_v4_network="$trial_dir/weakened-v4-network.md"
sed 's/^disabled, web search disabled/enabled, web search disabled/' "$decision" >"$weakened_v4_network"
if validate_protocol_v4_contract "$weakened_v4_network" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 with workspace network enabled" >&2
  exit 1
fi

weakened_v4_writable_roots="$trial_dir/weakened-v4-writable-roots.md"
sed 's/no additional writable roots/additional writable roots/' "$decision" >"$weakened_v4_writable_roots"
if validate_protocol_v4_contract "$weakened_v4_writable_roots" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 with extra writable roots" >&2
  exit 1
fi

weakened_v4_dynamic_tools="$trial_dir/weakened-v4-dynamic-tools.md"
sed 's/web search disabled, apps disabled, multi-agent tools disabled/web search enabled, apps enabled, multi-agent tools enabled/' "$decision" >"$weakened_v4_dynamic_tools"
if validate_protocol_v4_contract "$weakened_v4_dynamic_tools" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 with dynamic external tools enabled" >&2
  exit 1
fi

missing_v4_seed_boundary="$trial_dir/missing-v4-seed-boundary.md"
sed '/must contain no project Codex configuration that can vary the pair/d' "$decision" >"$missing_v4_seed_boundary"
if validate_protocol_v4_contract "$missing_v4_seed_boundary" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 without the project-config seed boundary" >&2
  exit 1
fi

missing_v4_non_authority="$trial_dir/missing-v4-non-authority.md"
sed '/Neither `LAUNCHER_BOUND` nor `INSTRUCTED_AND_OBSERVED` grants authority./d' "$decision" >"$missing_v4_non_authority"
if validate_protocol_v4_contract "$missing_v4_non_authority" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted launcher controls as authority" >&2
  exit 1
fi

missing_v4_keep_rule="$trial_dir/missing-v4-keep-rule.md"
sed '/`KEEP` is available under protocol v4 only if both remaining attempts/d' "$decision" >"$missing_v4_keep_rule"
if validate_protocol_v4_contract "$missing_v4_keep_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a missing protocol-v4 KEEP threshold" >&2
  exit 1
fi

missing_retention="$trial_dir/missing-retention.md"
sed '/evaluator identity and independence evidence, evaluator-packet path and/d' "$decision" >"$missing_retention"
if validate_trial_contract "$missing_retention" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted incomplete evaluator retention evidence" >&2
  exit 1
fi

reversed_v1_carry="$trial_dir/reversed-v1-carry.md"
sed 's/Protocol-v1 invalidity neither supports nor blocks a protocol-v2 `KEEP`;/Protocol-v1 invalidity supports a protocol-v2 `KEEP`;/' "$decision" >"$reversed_v1_carry"
if validate_trial_contract "$reversed_v1_carry" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol-v1 invalidity as KEEP evidence" >&2
  exit 1
fi

missing_classification_retention="$trial_dir/missing-classification-retention.md"
sed '/the case ID, protocol version, classification, user intent, and frozen/d' "$decision" >"$missing_classification_retention"
if validate_trial_contract "$missing_classification_retention" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing classification retention" >&2
  exit 1
fi

missing_verdict_retention="$trial_dir/missing-verdict-retention.md"
sed '/per-criterion and per-check outcomes with evidence hashes or deterministic/d' "$decision" >"$missing_verdict_retention"
if validate_trial_contract "$missing_verdict_retention" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing criterion and verdict retention" >&2
  exit 1
fi

misclassified_case_001="$trial_dir/misclassified-case-001.md"
sed 's/Terminal outcome: `INVALID`/Terminal outcome: `HARD_REGRESSION`/' "$trials" >"$misclassified_case_001"
if validate_case_001_receipt "$misclassified_case_001" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a misclassified case 001" >&2
  exit 1
fi

missing_case_001_invalidity="$trial_dir/missing-case-001-invalidity.md"
sed '/Invalidity evidence: the pre-run record did not bind exact writer budgets,/d' "$trials" >"$missing_case_001_invalidity"
if validate_case_001_receipt "$missing_case_001_invalidity" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing case-001 invalidity evidence" >&2
  exit 1
fi

misclassified_case_002="$trial_dir/misclassified-case-002.md"
sed 's/Terminal outcome: `INVALID`; no writer task was created/Terminal outcome: `PASS`; no writer task was created/' "$trials" >"$misclassified_case_002"
if validate_case_002_receipt "$misclassified_case_002" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted a misclassified case 002" >&2
  exit 1
fi

missing_case_002_host_gap="$trial_dir/missing-case-002-host-gap.md"
sed '/Invalidity evidence: task and subagent creation could bind model and effort,/d' "$trials" >"$missing_case_002_host_gap"
if validate_case_002_receipt "$missing_case_002_host_gap" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing case-002 host-gap evidence" >&2
  exit 1
fi

missing_case_002_permission_gap="$trial_dir/missing-case-002-permission-gap.md"
sed '/exposed no pre-start permission, sandbox, approval, credential, or network/d' "$trials" >"$missing_case_002_permission_gap"
if validate_case_002_receipt "$missing_case_002_permission_gap" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing case-002 permission-gap evidence" >&2
  exit 1
fi

missing_case_002_token_gap="$trial_dir/missing-case-002-token-gap.md"
sed '/policy binding, and the available goal token budget could be created only/d' "$trials" >"$missing_case_002_token_gap"
if validate_case_002_receipt "$missing_case_002_token_gap" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing case-002 token-gap evidence" >&2
  exit 1
fi

missing_v3_no_launch_receipt="$trial_dir/missing-v3-no-launch-receipt.md"
sed '/No canary case, writer task, implementation worktree, or real-session/d' "$trials" >"$missing_v3_no_launch_receipt"
if validate_protocol_v3_receipt "$missing_v3_no_launch_receipt" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v3 without a no-launch receipt" >&2
  exit 1
fi

missing_case_003_invalidity="$trial_dir/missing-case-003-invalidity.md"
sed '/Terminal outcome: `INVALID_PRELAUNCH`/d' "$trials" >"$missing_case_003_invalidity"
if validate_case_003_receipt "$missing_case_003_invalidity" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted missing case-003 invalidity evidence" >&2
  exit 1
fi

missing_v4_no_launch_receipt="$trial_dir/missing-v4-no-launch-receipt.md"
sed '/No v4 case, packet, writer, worktree, model invocation, or real-session/d' "$trials" >"$missing_v4_no_launch_receipt"
if validate_protocol_v4_receipt "$missing_v4_no_launch_receipt" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted protocol v4 without a no-launch receipt" >&2
  exit 1
fi

echo "GOALRAIL_INTENT_FIXTURE_CONTRACT_OK scope=structure-declared-boundary-independent-evaluator-launcher-bound-observable-controls-representative-identifiers"

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
  require_normalized_text 'Every normative expected behavior and forbidden change must have a stable criterion ID and be visible to both writers before they start.' "$file" || return 1
  require_normalized_text 'Only the implementation of an evaluator check and its concrete adversarial inputs may remain hidden. Every hidden check must cite one visible criterion ID and may test only that criterion; an evaluator must reject a hidden check that adds a normative requirement.' "$file" || return 1
  require_normalized_text '`KEEP` is available under protocol v2 only after at least two valid `DECISION_BEARING` pairs have observed fixture activation, independent acceptance, and a positive preselected intent-quality signal, with no protocol-v2 invalid pair or hard regression.' "$file" || return 1
  require_normalized_text 'the case ID, protocol version, classification, user intent, and frozen repository identity;' "$file" || return 1
  require_normalized_text 'evaluator identity and independence evidence, evaluator-packet path and content hash, stable criterion IDs, and the mapping from every hidden check to one visible criterion;' "$file" || return 1
  require_normalized_text 'deterministic commands, forbidden-change checks, budgets, pair attempt ceiling, stop condition, and execution order;' "$file" || return 1
  require_normalized_text 'verified actual model, effort, tools, permissions, fixture availability, and full-instruction activation evidence;' "$file" || return 1
  require_normalized_text 'the preselected intent-quality signal, its deterministic evaluation rule, observed value and verdict, supporting evidence identity, and whether useful existing approaches were found;' "$file" || return 1
  require_normalized_text 'per-criterion and per-check outcomes with evidence hashes or deterministic reproduction commands, independent evaluator verdict, and terminal outcome.' "$file" || return 1
  require_text 'Any invalid protocol-v2 pair prevents `KEEP` under protocol v2.' "$file" || return 1
}

validate_case_001_receipt() {
  file=$1

  require_text 'Terminal outcome: `INVALID`; the canary stopped after this pair.' "$file" || return 1
  require_normalized_text 'Invalidity evidence: the pre-run record did not bind exact writer budgets, per-pair attempt ceiling, execution order, or a content-hashed acceptance packet. The later sabotage was not preselected. Protocol v1 therefore cannot treat either writer result or the post-run comparison as outcome evidence.' "$file" || return 1
  require_normalized_text 'Because this check was not bound before either writer started, it motivated protocol v2 but is not accepted case-001 outcome evidence.' "$file" || return 1
  require_normalized_text 'Protocol v1 ended with `MODIFY`. Protocol v2 is a distinct owner-approved modified round with at most four additional pairs; case 001 must not be rerun.' "$file" || return 1
}

validate_contract "$skill"
validate_trial_contract "$decision"
validate_case_001_receipt "$trials"

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

reversed_invalid_rule="$trial_dir/reversed-invalid-rule.md"
sed 's/Any invalid protocol-v2 pair prevents `KEEP` under protocol v2./Any invalid protocol-v2 pair permits `KEEP` under protocol v2./' "$decision" >"$reversed_invalid_rule"
if validate_trial_contract "$reversed_invalid_rule" >/dev/null 2>&1; then
  echo "Goalrail intent trial accepted an invalid pair as KEEP evidence" >&2
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

echo "GOALRAIL_INTENT_FIXTURE_CONTRACT_OK scope=structure-declared-boundary-independent-evaluator-representative-identifiers"

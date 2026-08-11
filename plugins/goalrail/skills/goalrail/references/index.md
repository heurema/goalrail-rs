# Goalrail agent index

Use the CLI's own `--help` output for exact syntax and this index for intent
routing and evidence semantics. Public Goalrail CLI `0.3.0` and later supports
the summary, skills, and plugins workflows below. Still invoke only commands
advertised by the installed binary in `gr inspect codex --help`; a version
string alone is not capability evidence.

## Inspect a Codex environment

Use when the request is broad or ambiguous, such as "inspect Codex" or "help me
clean up Codex":

```sh
gr inspect codex --json
```

Use the summary and advertised drilldowns to choose a narrower follow-up. Do
not run every drilldown automatically.

## Review cleanup candidates among skills

Use when the user explicitly wants to clean or review skills:

```sh
gr inspect codex skills --actionable --json
```

Check `verdict`, `coverage`, `cleanupPolicy`, `cleanup`, and each item's
`cleanupDisposition`. Present `manual_review`, `defer`, and
`investigate_origin` separately. This command does not delete anything.

## Inspect the full active skill catalog

Use when the user asks for all active skills or last-use evidence:

```sh
gr inspect codex skills --json
```

Identity is `(name, manifestPath)`. Treat retained-rollout evidence as bounded
observation, not a complete usage ledger.

## Relate installed plugins to active skills

Use when the user asks which skills belong to plugins or wants plugin evidence:

```sh
gr inspect codex plugins --json
```

If the installed CLI does not advertise this workflow, report the capability
boundary instead of compiling from source or substituting another command.

Check `skillEvidence.observedAt`, thresholds, history coverage, link coverage,
invalid or duplicate identities, and counting bases. Lack of observed skill use
does not mean the plugin is unused.

## Currently unavailable drilldowns

Goalrail CLI has no dedicated MCP or marketplace drilldown and no
cleanup command. Their counts remain available in the broad Codex summary.
Report this boundary instead of inventing a command.

## Verdicts

- `BASELINE_OK` / exit `0`: required evidence was observed with no review
  finding.
- `REVIEW` / exit `1`: inspection completed and found something to review.
- `INCOMPLETE` / exit `3`: required evidence was absent, empty, or unsupported.
- `BLOCKED` / exit `4`: Codex or another required probe could not run.

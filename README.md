# Goalrail

Goalrail is a small Rust command-line tool for evidence-backed inspection of
coding-agent environments.

The current implementation inspects an existing Codex installation using its
native diagnostic commands and local configuration. It reports observed
findings without installing tools, changing configuration, or treating missing
evidence as success.

## Current command

```bash
cargo run -p gr -- inspect codex
cargo run -p gr -- inspect codex --json
cargo run -p gr -- inspect codex skills --actionable --json
cargo run -p gr -- inspect codex skills --json
cargo run -p gr -- inspect codex plugins --json
```

`gr inspect codex` checks the available Codex diagnostics, features,
instructions, plugins, the active skill count, MCP configuration,
marketplaces, and relevant project trust state. It returns one of four
verdicts:

| Verdict | Exit code | Meaning |
| --- | ---: | --- |
| `BASELINE_OK` | 0 | Required evidence was observed and no review finding was produced. |
| `REVIEW` | 1 | The inspection completed and found something that needs attention. |
| `INCOMPLETE` | 3 | Required evidence was missing, empty, or unsupported. |
| `BLOCKED` | 4 | Codex was unavailable or could not be executed. |

These verdicts are Goalrail policy, not native OpenAI classifications.
The exact Codex doctor limitation `terminal.env` with the known
`TERM=dumb - colors and cursor control are disabled` message is counted in
`doctor.ignoredCheckCount` but does not produce `REVIEW`; it is expected when
an agent runs the command without an interactive terminal. Other doctor
failures, including other `terminal.env` messages, remain actionable.

The summary refreshes the active skill count, breaks it down by ownership
origin, and advertises supported drilldowns without running their heavier
evidence scans. Its skills drilldown uses `--actionable` so an agent receives
only cleanup candidates by default. The command reports `itemView`,
`itemsReturned`, and `itemsOmitted`; remove `--actionable` to retrieve the full
catalog with last-use evidence.
`gr inspect codex skills --json` refreshes the active catalog
through Codex `skills/list`, then scans retained rollout files for exact skill
manifest paths in tool calls and counts at most one observation per task turn.
It excludes the current Codex task, reports the observed history window,
partial-read counters, and known undercounting limitations, and never calls an
unobserved skill "unused". Each item includes its manifest path and classifies
its ownership origin as system, plugin, personal, project, admin, or unknown so
agents do not treat plugin-managed skills as personal cleanup candidates. The
item identity is `(name, manifestPath)`, so project and global skills with the
same name remain separate. The scan has a 15-second ceiling; a bounded result
sets `coverage.truncated` instead of silently extending the run. No usage cache
or durable memory is written.

Usage signals remain factual observations. Cleanup policy is exposed
separately through `cleanupPolicy`, the `cleanup` count summary, and each
item's `cleanupDisposition`. Plugin, system, and admin skills are
`managed_no_manual_cleanup`; owner-managed recent skills are `keep`, aging or
unobserved skills are `manual_review`, insufficient evidence is `defer`, and
unknown ownership is `investigate_origin`. Findings use
`skills.cleanup.review`, `skills.cleanup.deferred`, and
`skills.cleanup.origin_unknown` for the actionable cases. This intentionally
changes verdict semantics: stale managed skills no longer produce `REVIEW` by
themselves, while owner-managed cleanup candidates, insufficient evidence, or
unknown ownership still do.

`gr inspect codex plugins --json` returns the installed plugin inventory from
the native `codex plugin list --json` surface and joins it to the active
plugin-origin skills from the same bounded skill-evidence pipeline used by the
skills drilldown. The internal records for plugins, skills, links, and observed
uses remain separate; the JSON nests linked skills under each plugin for agent
convenience. A link is accepted only when the exact skill manifest path falls
under exactly one plugin root: either native `source.path` or the installed
cache root derived from `CODEX_HOME` and the native `(marketplace, name,
version)` identity. Unlinked and ambiguous skills are reported explicitly and
produce `REVIEW` rather than a guessed relation.

Items are sorted by exact plugin ID and report the plugin name, marketplace,
version, enabled state, auth policy, active linked skills, observed-use counts,
and latest observed skill use. `observedAt`, signal thresholds, history and link
coverage are included so an agent can judge the evidence. Per-skill counts use
unique task turns; the plugin total explicitly reports that it is the sum of
those per-skill counts and can therefore count one turn more than once when it
uses several skills. Invalid path components or duplicate plugin IDs produce
`REVIEW` and are listed instead of being linked optimistically. Disabled
plugins remain factual inventory and do not produce `REVIEW`; lack of observed
skill use never means that a plugin is unused because plugins can also provide
MCP, apps, and other non-skill capabilities. The command does not recommend
cleanup, expose the available marketplace catalog, or enable, disable, install,
or remove anything. It writes no cache, durable memory, or database.

## Codex plugin trial

The repository packages a Codex-only, skills-only Goalrail plugin under
`plugins/goalrail` and exposes it through the repo marketplace at
`.agents/plugins/marketplace.json`. Its current checked-in state is
packaging-only: the marketplace is not registered and the plugin is not
installed merely because these files exist. Once explicitly activated, the
plugin is the agent-facing intent router; the separately installed `gr` binary
remains the evidence engine. The trial does not bundle, install, update, or
remove the binary automatically.

The trial channel is installed from the remote Git marketplace on `main` using
two separate Codex configuration changes:

```bash
codex plugin marketplace add heurema/goalrail-rs --ref main --json
codex plugin add goalrail@goalrail --json
```

Each command requires its own explicit approval and JSON readback; marketplace
registration does not authorize plugin installation. These commands are not
run by `mise run test:goalrail-plugin` or `mise run ci`.

The marketplace entry uses the remote `git-subdir` source and pins
`plugins/goalrail` to the immutable plugin tag `plugin-v0.1.0`. The marketplace
catalog can keep refreshing from `main` without making the installed plugin
follow the branch automatically. Every agent-behavior change bumps the plugin
version and advances the entry to the matching `plugin-v<version>` tag.

Run `mise run test:goalrail-plugin` for offline package validation. After the
exact ref has been pushed, run the real Git-backed smoke test with:

```bash
mise run smoke:goalrail-plugin-remote -- main 0.1.0
```

The smoke test uses an isolated temporary `CODEX_HOME`; it does not register or
install the plugin in the operator's normal Codex profile. It verifies the Git
marketplace source, exact remote payload URL/path/tag and resolved tag commit,
installed plugin identity and version, and that Codex `skills/list` exposes the
`goalrail` skill from the installed plugin cache.
The public `v0.2.0` binary supports the summary and skills flows but predates
the plugins drilldown. The trial skill checks the installed CLI help surface
and labels that workflow as source-preview until the next release.

## Design boundaries

Goalrail is currently a modular monolith:

- `gr` is a thin CLI adapter;
- `gr-inspect-codex` owns inspection orchestration and reporting;
- library internals remain private behind a narrow use-case facade.

The durable boundaries and their enforcement are documented in
[ARCHITECTURE-SPINE.md](ARCHITECTURE-SPINE.md).

## Development

The workspace uses Rust 1.97.1 through `mise`.

```bash
mise run setup
mise run ci
```

`mise run ci` checks formatting, Clippy, the narrow compiler-enforced skill
assessment boundary, workspace tests, coverage, and local verification-receipt
behavior. Other architecture invariants still require explicit review; the
retired custom checker must not be treated as a passing gate.
`mise run architecture:public-api` is a separate trial that compares only the
documented rustdoc-visible facade slice and does not prove the architecture
spine. `mise run architecture:drift` is a separate advisory aggregate review:
it reports exact workspace-edge, facade-item, Rust-file content, and per-file
line-count movement as structured `NO_CHANGE` or `REVIEW` JSON. It does not parse internal
Rust dependencies or prove architecture conformance, and it never updates its
accepted baseline automatically. A successful
`verify:rust-milestone` run records local verification evidence under `.git`.
The tracked native pre-push hook checks every pushed ref for exact-tree CI
evidence and a full-tree verification receipt chain; it does not run CI or
mutation tests while a network connection is open. Missing exact-tree evidence
prints a fast `verify:ci-state <base> <head>` recovery command only when the
missing edge changes no Rust source. Any unverified `.rs` change prints the
goal-scoped `verify:outgoing-rust` command to run before retrying the push.

## Local release preparation

The repository can prepare and verify a macOS arm64 release bundle locally
without publishing it:

```bash
mise run release:test
mise run release:prepare -- 0.2.0
mise run release:check -- 0.2.0
```

The generated bundle under `dist/v0.2.0/` contains the binary and MIT license
archive, its SHA-256, a release manifest, and a rendered Homebrew formula.
`dist/` is ignored and is never a source of truth.

Before any public release, run the read-only source gate:

```bash
mise run release:preflight -- 0.2.0
```

It fails closed unless the version matches, the checkout is clean and tagged,
the host is macOS arm64, and distribution terms have been selected. Passing the
gate does not publish, tag, install, or update anything.

## Design documents

- [Native-first decision](docs/decisions/0001-keep-goalrail-native-first.md)
  records why Goalrail begins with a read-only adapter over supported Codex
  capabilities.
- [Agent-first lifecycle decision](docs/decisions/0002-make-lifecycle-agent-first.md)
  defines the bounded Homebrew protocol for agent-operated installation,
  update, and removal.
- [Local verification receipt trial](docs/decisions/0003-use-local-verification-receipts-for-push.md)
  separates slow local verification from the pre-push network operation.
- [Retired architecture fitness trial](docs/decisions/0004-trial-native-architecture-fitness.md)
  records why the repository-owned Ruby checker was removed and what a future
  replacement must prove.
- [Skill evidence-to-assessment boundary](docs/decisions/0005-propose-skill-evidence-assessment-boundary.md)
  adopts AD-6 and records the first pure assessment extraction without changing
  the public contract.
- [Public API facade trial](docs/decisions/0006-trial-cargo-public-api.md)
  records the exact facade slice checked by `cargo-public-api` and its known
  blind spots.
- [Rejected cargo-pup dependency gate](docs/decisions/0007-reject-cargo-pup-dependency-gate.md)
  records the qualified-path false accept that prevents a dependency-direction
  or AD-6 claim.
- [Compiler-enforced skill assessment boundary](docs/decisions/0008-enforce-skill-assessment-crate-boundary.md)
  records the narrow internal crate extraction used instead of a source-level
  import policy.
- [Identity-based architecture drift trial](docs/decisions/0009-trial-identity-based-architecture-drift.md)
  records the advisory aggregate snapshot, its agent-facing JSON contract, and
  why it is not an architecture-conformance gate.
- [In-memory plugin-skill evidence decision](docs/decisions/0010-link-plugin-skill-evidence-in-memory.md)
  records the normalized relation used by the plugins drilldown and why SQLite
  remains deferred.
- [Co-located Codex plugin trial](docs/decisions/0011-co-locate-goalrail-codex-plugin.md)
  keeps the skills-only plugin, agent routing, and repo marketplace beside the
  CLI while preserving a separate binary distribution path.
- [Model behavior evaluation proposal](docs/ideas/model-behavior-evaluation.md)
  captures a possible provider-neutral comparison mechanism. It is not part of
  the runtime or public CLI yet.
- [Historical v0 candidate](docs/history/inspect-codex-v0-candidate.md) preserves
  the original pre-implementation contract and explicitly lists where the
  shipped behavior differs.

## Status and license

Goalrail is experimental and has no stability guarantee. It is available under
the [MIT License](LICENSE).

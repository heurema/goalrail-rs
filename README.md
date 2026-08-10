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
cargo run -p gr -- inspect codex skills --json
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

The summary refreshes the active skill count, breaks it down by ownership
origin, and advertises supported drilldowns without running their heavier
evidence scans.
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

`mise run ci` checks formatting, runs Clippy with warnings denied, executes the
workspace tests, and reports coverage. The installed pre-push hook runs the
same checks plus mutation testing for changed Rust files.

## Local release preparation

The repository can prepare and verify a macOS arm64 release bundle locally
without publishing it:

```bash
mise run release:test
mise run release:prepare -- 0.1.0
mise run release:check -- 0.1.0
```

The generated bundle under `dist/v0.1.0/` contains the binary and MIT license
archive, its SHA-256, a release manifest, and a rendered Homebrew formula.
`dist/` is ignored and is never a source of truth.

Before any public release, run the read-only source gate:

```bash
mise run release:preflight -- 0.1.0
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
- [Model behavior evaluation proposal](docs/ideas/model-behavior-evaluation.md)
  captures a possible provider-neutral comparison mechanism. It is not part of
  the runtime or public CLI yet.
- [Historical v0 candidate](docs/history/inspect-codex-v0-candidate.md) preserves
  the original pre-implementation contract and explicitly lists where the
  shipped behavior differs.

## Status and license

Goalrail is experimental and has no stability guarantee. It is available under
the [MIT License](LICENSE).

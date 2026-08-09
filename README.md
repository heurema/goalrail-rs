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
```

`gr inspect codex` checks the available Codex diagnostics, features,
instructions, plugins, MCP configuration, marketplaces, and relevant
project trust state. It returns one of four verdicts:

| Verdict | Exit code | Meaning |
| --- | ---: | --- |
| `BASELINE_OK` | 0 | Required evidence was observed and no review finding was produced. |
| `REVIEW` | 1 | The inspection completed and found something that needs attention. |
| `INCOMPLETE` | 3 | Required evidence was missing, empty, or unsupported. |
| `BLOCKED` | 4 | Codex was unavailable or could not be executed. |

These verdicts are Goalrail policy, not native OpenAI classifications.

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

## Status and license

Goalrail is experimental and has no stability guarantee. No software license
has been selected yet; public visibility does not grant permission to reuse,
modify, or redistribute the code.

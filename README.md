# Goalrail

Goalrail is a small Rust command-line tool for evidence-backed inspection of
coding-agent environments.

The current implementation inspects an existing Codex or Claude Code
installation using its native diagnostic commands and local configuration. It
reports observed findings without installing tools, changing configuration, or
treating missing evidence as success.

## Current command

```bash
cargo run -p gr -- inspect codex
cargo run -p gr -- inspect codex --json
cargo run -p gr -- inspect codex skills --actionable --json
cargo run -p gr -- inspect codex skills --json
cargo run -p gr -- inspect codex plugins --json
cargo run -p gr -- inspect codex updates --json
cargo run -p gr -- inspect claude
cargo run -p gr -- inspect claude --json
```

`gr inspect codex` checks the available Codex diagnostics, features,
instructions, plugins, the active skill count, MCP configuration,
marketplaces, and relevant project trust state. It returns one of four
verdicts:

The summary reports `codexVersion` from the native CLI and `appVersion` from
`/Applications/Codex.app/Contents/Info.plist` (`CFBundleShortVersionString`).
`appVersion` is `null` when the desktop app is not installed or its metadata is
not available; that absence does not change the inspection verdict.

`gr inspect codex updates --json` is a separate, explicit network request. It
compares the installed Codex CLI semantic version with the official `latest`
metadata for `@openai/codex` at `registry.npmjs.org`. The probe uses the
existing `curl` executable with bounded network and process timeouts; missing,
unreachable, or invalid source data returns `INCOMPLETE`. Availability states
report discovery only and never authorize an update.

The desktop app channel reports its installed version, but leaves
`availableVersion` and `source` as `null`, with `freshness: UNVERIFIED`, until
an official machine-readable latest-version source is verified. The command
does not scan issues, infer whether an update is safe, launch an updater, or
change configuration. Ordinary `gr inspect codex` remains local and only
advertises this opt-in drilldown.

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

## Claude Code inspection

`gr inspect claude` reports the same four verdicts from a different evidence
surface. It runs exactly three native commands — `claude --version`,
`claude plugin list --json`, and `claude plugin marketplace list --json` — and
reads the documented configuration under the resolved Claude home, which is
`CLAUDE_CONFIG_DIR` when set and `~/.claude` otherwise. It writes nothing.

The summary reports the Claude version, the installed plugin inventory with
enabled and per-scope counts, the configured marketplace count, MCP servers
counted from `~/.claude.json` and the project `.mcp.json` by scope, personal and
project skill manifests found on disk, the resolved Claude home, the detected
project root, and the discovered `CLAUDE.md` instruction sources with their
scope and size. An instruction source is reported as discovered, not as loaded:
the command observes that the file exists and how large it is.

Claude Code records local-scope state under the directory a session was started
in, so `stateEntry` is keyed on the current working directory rather than on the
project root, and `currentDir` and `stateEntryPath` name the exact directory and
the exact matched `projects` key. Local-scope MCP servers are counted from that
same entry.

Claude Code exposes less machine-readable diagnostic surface than Codex, so the
report names each gap in `evidenceLimitations` instead of guessing:
`claude doctor` has no JSON form and is not inspected; `claude mcp list` has no
JSON form, so MCP servers are counted from configuration without connection,
health, or authentication state; the skill counts cover personal and project
`SKILL.md` manifests only, not bundled or plugin-contributed skills; and no
usage history is observed, so nothing in the report means a plugin or skill is
unused. There is no skills or plugins drilldown for Claude: a Claude transcript
records a skill invocation by name rather than by manifest path, so the exact
manifest-path evidence basis used by the Codex drilldowns has no equivalent.

Absent configuration is evidence of zero rather than failure. `REVIEW` is
reserved for observable inconsistencies in the native inventory: an installed
plugin whose recorded `installPath` does not exist or cannot be examined, and a
plugin id reported more than once. A failed, timed-out, or unparsable probe and
malformed configuration are `INCOMPLETE`; an unavailable `claude` executable is
`BLOCKED`. A configuration file that exists but cannot be read fails the
inspection rather than being counted as empty. For the project root marker,
skill manifests, and instruction files the command examines metadata only, so a
path whose metadata cannot be examined fails the inspection, while a path that
resolves to nothing — including a dangling symlink — reads as absent. File
contents are never opened, so the report does not assert that a discovered
manifest or instruction file is readable.

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
`plugins/goalrail` to the immutable shared release tag `v0.3.10`. The catalog can
refresh from `main`, while the payload remains immutable. Live canaries observed
the current Codex host reconcile the catalog and installed cache both at task
startup and during `marketplace upgrade`, so neither operation is documented as
a metadata-only update check. CLI crates and the plugin share one release
version. A behavior change in either advances the whole Goalrail release and the
entry to the matching `v<version>` tag; offline validation rejects drift. The
release runbook creates and publishes that tag before promoting the catalog
commit to `main`.

Run `mise run test:goalrail-plugin` for offline package validation. After the
exact ref has been pushed, run the real Git-backed smoke test with:

```bash
mise run smoke:goalrail-plugin-remote -- main 0.3.10
```

The smoke test uses an isolated temporary `CODEX_HOME`; it does not register or
install the plugin in the operator's normal Codex profile. It verifies the Git
marketplace source, exact remote payload URL/path/tag and resolved tag commit,
installed plugin identity and version, and that Codex `skills/list` exposes the
`goalrail` skill from the installed plugin cache.
Public CLI `v0.3.0` and later supports the summary, skills, and plugins flows.
The skill still checks the installed CLI help surface before routing because a
version string alone is not capability evidence.

## Design boundaries

Goalrail is currently a modular monolith:

- `gr` is a thin CLI adapter;
- `gr-inspect-codex` owns inspection orchestration and reporting;
- the internal `gr-skill-assessment` crate owns pure skill assessment policy
  without process, filesystem, environment, clock, or rendering access;
- `gr-site` enhances the static site without depending on inspection crates;
- library internals remain private behind a narrow use-case facade.

The durable boundaries and their enforcement are documented in
[ARCHITECTURE.md](ARCHITECTURE.md).

## Development

The workspace uses Rust 1.97.1 through `mise`.

```bash
mise run setup
mise run ci
```

`mise run ci` checks formatting, Clippy, the narrow compiler-enforced skill
assessment boundary, the Goalrail plugin and release packaging, workspace
tests, coverage, and local verification-receipt behavior. Other architecture
invariants still require explicit review; the
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

## Release

Goalrail packages macOS arm64, Linux x86_64, and Windows x86_64 binaries. The
native GitHub workflow verifies one exact remote commit on all three platforms
before an immutable tag is created. It emits a candidate only and has no tag,
release, branch, or Homebrew write permission.

The canonical agent procedure, approval boundaries, recovery rules, exact
commands, and public asset digest checks are in
[`docs/release.md`](docs/release.md). A successful candidate is not public
availability. Homebrew remains the managed macOS arm64 path; Windows artifacts
are unsigned, and Linux and Windows have no package-manager lifecycle yet.

## Design documents

- [Public site product brief](PRODUCT.md) defines the landing-page audience,
  promise, and design principles.
- [Public site design brief](DESIGN.md) defines the visual, interaction, motion,
  and accessibility contract for `gr-site`.
- [Native-first decision](docs/decisions/0001-keep-goalrail-native-first.md)
  records why Goalrail begins with a read-only adapter over supported Codex
  capabilities.
- [Agent-first lifecycle decision](docs/decisions/0002-make-lifecycle-agent-first.md)
  defines the bounded Homebrew protocol for agent-operated installation,
  update, and removal.
- [Local verification receipt policy](docs/decisions/0003-use-local-verification-receipts-for-push.md)
  records the completed trial, retained fail-fast policy, and reopen conditions.
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
- [Native multi-platform release bundle](docs/decisions/0012-build-native-multi-platform-release-bundles.md)
  adds Windows and Linux assets without coupling build authority to GitHub
  Release or Homebrew publication.
- [Hardened Homebrew release stage](docs/decisions/0013-harden-homebrew-release-stage.md)
  replaces repeated Homebrew parsing and formula-push shell fragments with two
  narrow fail-closed contracts while preserving separate release approvals.
- [Model behavior evaluation proposal](docs/ideas/model-behavior-evaluation.md)
  defers a possible provider-neutral comparison mechanism until a named
  consumer and a reproducible receipt-backed comparison case both exist.
- [Historical v0 candidate](docs/history/inspect-codex-v0-candidate.md) preserves
  the original pre-implementation contract and explicitly lists where the
  shipped behavior differs.

## Status and license

Goalrail is experimental and has no stability guarantee. It is available under
the [MIT License](LICENSE).

---
name: goalrail
description: Use Goalrail when the user wants to inspect, diagnose, or clean a Codex environment, skills, plugins, MCP configuration, marketplaces, or project trust, or explicitly manage the Goalrail CLI package or Goalrail Codex plugin. Route the intent to the narrowest supported workflow, interpret structured evidence, distinguish binary lifecycle from plugin lifecycle, and preserve explicit authority for system changes.
---

# Goalrail

Goalrail is the read-only evidence engine. The skill selects a workflow and
explains evidence; it does not turn inspection into permission to change the
host.

1. Read `references/index.md` and choose the narrowest workflow matching the
   user's intent. Do not run the summary first when a specific drilldown is
   already clear.
2. Before a Goalrail command, resolve `gr` with `command -v gr`, inspect
   `gr --version`, and read `gr inspect codex --help`. Route only to a
   subcommand advertised by that installed binary. Public CLI `0.2.0` supports
   the summary and skills flows; the first shared CLI/plugin release is planned
   as `0.3.0`, including the plugins drilldown. Until that release exists,
   treat the drilldown as a source-preview capability. If `gr` is absent, read
   `references/install.md`, perform only its read-only preflight, and offer the
   native CLI installation as a separate approval-gated action.
3. Resolve lifecycle intent before reading instructions:
   - for the native `gr` CLI or Homebrew package, read
     `references/install.md`;
   - for the Goalrail Codex plugin or marketplace, read
     `references/plugin-lifecycle.md`;
   - if the user says only "Goalrail", ask which target they mean instead of
     inferring it.
   Treat each state-changing command as a separate action requiring exact
   approval for its named target.
4. Prefer `--json`. Interpret Goalrail verdicts and coverage before presenting
   conclusions. Missing evidence never means success.
5. Never call an unobserved skill unused. Never recommend plugin removal from
   skill-use evidence alone; plugins can also provide MCP, apps, or hooks.
6. Goalrail commands are read-only. Any later cleanup or configuration change
   is outside the inspection command and requires the exact target and separate
   operator approval.
7. If the requested capability is absent from the index or the installed CLI,
   say so. Do not invent a command or silently substitute another tool.

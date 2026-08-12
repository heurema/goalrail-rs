---
name: goalrail
description: Use Goalrail when the user wants to inspect, diagnose, or clean a Codex environment, skills, plugins, MCP configuration, marketplaces, or project trust, check for Goalrail updates, or explicitly manage the Goalrail CLI package or Goalrail Codex plugin. Route the intent to the narrowest supported workflow, interpret structured evidence, distinguish binary lifecycle from plugin lifecycle, and preserve explicit authority for system changes.
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
   subcommand advertised by that installed binary. Public CLI `0.3.0` and later
   supports the summary, skills, and plugins flows. Keep capability detection:
   a version string alone does not prove the installed command surface.
   If `gr` is absent, read
   `references/install.md`, perform only its read-only preflight, and offer the
   native CLI installation as a separate approval-gated action.
3. Resolve lifecycle intent before reading instructions:
   - for an explicit update check, read `references/update-discovery.md` and
     inspect both independently managed channels unless the user named one;
   - for an ambiguous request to "update Goalrail", perform only the
     observation flow in that reference, disclose the Codex host
     reconciliation boundary, then ask which evidenced channel to change;
   - for the native `gr` CLI or Homebrew package, read
     `references/install.md`;
   - for the Goalrail Codex plugin or marketplace, read
     `references/plugin-lifecycle.md`;
   - for any other lifecycle mutation where the user says only "Goalrail", ask
     which target they mean instead of inferring it.
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

# Goalrail Agent Rules

- Communicate with the owner in Russian.
- Use English for code, comments, filenames, commands, commits, and technical
  documentation.
- Before changing crate boundaries, dependency direction, public APIs, or CLI
  command ownership, read `ARCHITECTURE-SPINE.md` and preserve its invariants.
  If an invariant must change, update the spine before implementation.
- Keep one diff focused on one purpose and preserve unrelated owner work.
- Run `mise run ci` after Rust behavior or architecture changes.
- Do not commit, push, publish, release, deploy, or perform destructive work
  without explicit owner approval.

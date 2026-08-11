# Trial cargo-public-api for Facade Drift

- **Status:** trial
- **Date:** 2026-08-11
- **Decision owner:** t3chn

## Decision Question

Can a maintained Rust-aware tool detect accumulated drift in the
`gr-inspect-codex` facade without claiming to prove the complete architecture
spine?

## Decision

Trial `cargo-public-api` 0.52.0 as a separate, manually invoked snapshot gate.
It is pinned in `mise.toml` but is not part of `mise run ci`.

The accepted claim is deliberately narrow: for `gr-inspect-codex`, the task
detects changes to rustdoc-visible public declarations and explicit impls with
no default features on `aarch64-apple-darwin`. It fails closed when the tool or
nightly version differs, rustdoc references are unresolved, or the generated
surface differs from the checked-in snapshot.

The task omits blanket, auto-trait, and auto-derived impls because the initial
Goalrail run produced 21 unresolved rustdoc references when auto traits were
included. It therefore does not protect `Send`, `Sync`, other auto-trait drift,
or derived impl drift. It also does not see `#[doc(hidden)] pub` items and does
not prove serialization compatibility, crate dependency direction, AD-6 stage
direction, or assessment purity.

## Trial Evidence

The initial isolated canary used `cargo-public-api` 0.52.0 and the rolling
`nightly` alias, which resolved to
`rustc 1.99.0-nightly (1a98b1e13 2026-08-07)`. The accepted task pins
`nightly-2026-08-07`, which resolves to
`rustc 1.99.0-nightly (84b36a78a 2026-08-06)`; it produced the same accepted
24-line snapshot.

| Change | Full output | Accepted trial mode |
| --- | --- | --- |
| Same input repeated | stable | stable |
| Private-only item | stable | stable |
| Public function addition | detected | detected |
| Public signature change | detected | detected |
| Enum variant addition | detected | detected |
| Explicit trait impl | detected | detected |
| Public re-export | detected | detected |
| Macro-generated public item | detected | detected |
| Auto-trait loss | detected | intentionally omitted |
| `non_exhaustive` removal | detected | detected |
| `#[doc(hidden)] pub` addition | not detected | not detected |

The repository check has sabotage tests for a stale snapshot, unresolved
rustdoc references, and an unexpected tool version. Normal CI remains unchanged
while the cost and signal quality are observed.

## Operator Contract

Run:

```bash
mise run setup
mise run architecture:public-api
```

If an intentional facade change produces a diff, review the contract first and
then regenerate the snapshot with the exact pinned toolchain. Do not accept a
snapshot update merely to make the task green.

## Revisit Condition

After three real facade-change reviews or by 2026-09-11, choose `KEEP`,
`MODIFY`, `MOVE`, or `REMOVE`. Revisit immediately if the pinned nightly becomes
unavailable, the task reports unresolved IDs, the target or feature set changes,
or a missed public-contract regression is found.

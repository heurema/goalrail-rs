# Decision 0012: Build native multi-platform release bundles

- Status: accepted; local tooling implemented, native runner canary pending
- Date: 2026-08-11
- Owner: project owner

## Decision question

How should Goalrail add Windows and Linux binaries without weakening the
existing separation between tag creation, GitHub Release publication, and the
Homebrew tap update?

## Constraints and verified facts

- The public `v0.3.1` release and lifecycle documentation support macOS arm64
  through Homebrew only.
- Existing local tooling builds, executes, packages, and verifies only
  `aarch64-apple-darwin`.
- A release tag push and a GitHub Release are separately authorized operations.
  A tag event must not silently gain release-publication authority.
- Goalrail has no native dependencies beyond the Rust standard library, but a
  successful cross-compilation alone does not prove that process execution and
  path handling work on the target host.
- GitHub provides native `macos-15` arm64, `ubuntu-22.04` x86_64, and
  `windows-2025` x86_64 runners. Runner images can drift even when their OS
  labels are fixed.
- GitHub workflow permissions and action refs are repository-owned supply-chain
  inputs. Adjacent checksum files detect corruption but do not prove publisher
  authenticity.
- Homebrew remains the package owner only for macOS arm64. Goalrail has not
  selected a Linux or Windows package manager, installer, signing identity, or
  updater.

## Options

1. Publish a release automatically whenever a `v*` tag is pushed. This is
   convenient but collapses two previously separate owner-authority steps.
2. Use a manually dispatched workflow that both builds and publishes. This
   preserves an explicit trigger, but publication safety would also depend on
   an externally configured protected GitHub Environment.
3. Use a manually dispatched, read-only workflow that produces a verified
   bundle only. Keep GitHub Release creation and Homebrew tap update as later,
   separately approved actions.

## Decision

Select option 3.

The workflow accepts exactly one existing annotated stable tag. Its first job
resolves that tag to a full commit SHA and workspace version. Every later job
checks out the SHA rather than the tag name and rechecks that the tag still
resolves to the selected commit. Build jobs use `Cargo.lock`, `--locked`, and
the exact Rust version declared in `mise.toml`.

The first target set is deliberately narrow:

| Target | Native runner | Distribution status |
| --- | --- | --- |
| `aarch64-apple-darwin` | `macos-15` | GitHub asset and Homebrew input |
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` | direct GitHub asset |
| `x86_64-pc-windows-msvc` | `windows-2025` | direct unsigned GitHub asset |

This does not claim all Linux distributions, Windows on ARM, Linux on ARM, a
package-manager lifecycle, or Windows code signing. Ubuntu 22.04 is the first
native Linux build-and-smoke floor. Windows users may see SmartScreen warnings
until a signing decision is made.

Each native job must:

1. build `gr` for its matching target;
2. execute the binary and require `gr <version>`;
3. execute `gr inspect codex --json` against a compiled cross-platform Codex
   fixture and require `BASELINE_OK` with expected summary evidence;
4. package only the MIT license and native binary;
5. verify the archive, checksum, and exact target manifest.

The aggregate bundle records the tag, full source commit, Cargo.lock SHA-256,
target identities, artifact digests and sizes, Rust release/host, and runner
image metadata. The assembly job refuses missing targets, mixed commits,
mixed lock digests, extra manifest fields, checksum drift, target-to-rustc-host
drift, wrong executable formats or architectures, or a mismatched Homebrew
formula. The downloadable bundle retains the three target manifests because
offline revalidation depends on them. Formula syntax verification requires
Ruby and fails closed when Ruby is unavailable.

The workflow uses `contents: read`, disables persisted checkout credentials,
and pins third-party actions to full commit SHAs with version comments. It
uploads a 14-day Actions artifact and explicitly reports
`READY_FOR_PUBLICATION`; it has no GitHub Release or Homebrew write path.

## Independent critique and resolved objections

Independent review rejected the original build-and-publish workflow because a
plain `workflow_dispatch` does not itself prove owner approval and because a
protected release environment would be an untracked external dependency. The
selected build-only workflow removes that authority ambiguity.

The review also required one resolved tag SHA across all jobs, native execution
on every runner, atomic three-target assembly, a defined Linux floor, pinned
actions, recorded build provenance, and an honest unsigned-Windows limitation.
Those requirements are part of the adopted contract.

Native fixture smoke is evidence that Goalrail's Codex subprocess contract
works on the runner. It is not evidence that every real Codex version or host
configuration works. The first public multi-platform release still requires a
real workflow canary, downloaded-bundle verification, and post-publication
asset checks.

The assembly job resolves the annotated tag from the remote again immediately
before emitting `READY_FOR_PUBLICATION`. Post-download verification repeats the
same remote peeled-tag comparison before the separately approved publication,
so a moved or replaced tag cannot be hidden by checkout's local ref snapshot.

## Publication and rollback

Dispatching the workflow, creating the GitHub Release, and updating the
Homebrew tap each require their own exact owner approval. Publication must use
the complete verified bundle from one successful run. An existing release or
asset name is a hard collision; no file is replaced or uploaded with clobber
semantics.

A build or assembly failure publishes nothing. A defect discovered after
publication is corrected with a higher patch release; the immutable tag and
assets are not moved or replaced. The Homebrew tap remains unchanged until its
separate approved update.

## Revisit condition

Revisit when a real user needs Linux arm64, Windows arm64, an older Linux ABI
floor, package-manager ownership on Linux or Windows, Windows signing, release
attestations, or when runner-image drift makes the current provenance receipt
insufficient.

# Decision 0012: Build native multi-platform release bundles

- Status: accepted; native candidate passed, run-readback amendment pending repeat canary
- Date: 2026-08-11
- Owner: project owner

## Decision question

How should Goalrail build and verify Windows, Linux, and macOS binaries before
creating an immutable tag, without weakening the separation between candidate
verification, tag creation, GitHub Release publication, and Homebrew update?

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
- Four real native workflow runs for `v0.3.2` through `v0.3.5` failed before
  publication on independent portability or release-contract defects. Because
  the workflow required an existing immutable tag, each failure consumed a
  version even though no GitHub Release was created.

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

The first native canaries exposed a second decision within that build-only
option:

1. Keep requiring an immutable tag before native verification. Rejected because
   every pre-publication portability failure consumes a public version name.
2. Build from a mutable candidate tag and move or delete it after verification.
   Rejected because mutable tag semantics weaken identity and leave ambiguous
   recovery behavior.
3. Build from one exact remote commit, emit a candidate receipt, and create the
   immutable tag only after the complete candidate passes. Selected.

The build-only workflow accepts exactly one full source commit SHA before a tag
exists. The workflow-dispatch ref, GitHub's resolved workflow SHA, and the input
source SHA must be identical, so an agent cannot run reviewed source through a
different workflow definition. Every later job checks out that exact commit.
The workspace version defines one planned stable tag, and both the resolve and
aggregate jobs require that tag to remain absent on the remote. Build jobs use
`Cargo.lock`, `--locked`, and the exact Rust version declared in `mise.toml`.

The workflow emits `READY_FOR_TAG`, not `READY_FOR_PUBLICATION`. A successful
candidate binds the planned tag, source commit, workflow run ID and attempt,
workflow name, dispatch event, candidate branch, Cargo.lock digest, target
manifests, runner metadata, and native artifacts. It does not create or push a
tag, create a GitHub Release, update `main`, or modify Homebrew.

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

The aggregate bundle records the planned tag, full source commit, Cargo.lock SHA-256,
target identities, artifact digests and sizes, Rust release/host, and runner
image metadata. The assembly job refuses missing targets, mixed commits,
mixed lock digests, extra manifest fields, checksum drift, target-to-rustc-host
drift, wrong executable formats or architectures, or a mismatched Homebrew
formula. The downloadable bundle retains the three target manifests because
offline revalidation depends on them. Formula syntax verification requires
Ruby and fails closed when Ruby is unavailable.

The workflow uses `contents: read`, disables persisted checkout credentials,
and pins third-party actions to full commit SHAs with version comments. It
uploads a 14-day Actions artifact and explicitly reports `READY_FOR_TAG`; it has
no tag, GitHub Release, branch, or Homebrew write path.

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

The assembly job rechecks that the planned tag is absent immediately before
emitting `READY_FOR_TAG`. The agent then downloads one explicitly selected run,
verifies the bundle against its recorded source commit, creates and pushes the
annotated tag only after separate approval, and checks the remote peeled tag
before any publication step.

An independent authority review requested signed attestations, tag-protection
rules, and a write-capable promotion workflow. Those are not adopted in this
amendment because they add external configuration and release authority beyond
the current build-only trial. The repository continues to state that checksums
and manifests prove consistency, not publisher authenticity. Revisit those
controls before multiple release operators or automated publication are added.

The review also found two gaps that are adopted here:

- a transport or draft-upload failure may resume against the same tag only when
  every already uploaded asset is expected and its size and digest match the
  verified candidate; a partial-draft readback supplies the exact missing-asset
  allowlist before any resume upload;
- public readback must compare the exact asset set, sizes, and GitHub-reported
  SHA-256 digests, rather than treating asset-name presence as publication proof.
  It also revalidates that the remote annotated tag still resolves to the source
  commit recorded by the verified aggregate manifest and that the manifest's
  run ID and attempt equal the explicitly selected workflow run.

The first live pre-tag canary completed successfully for all three targets and
the aggregate bundle. Its post-run readback exposed a GitHub CLI identity trap:
`gh run view --json workflowName` returned the old default-branch registry name,
while the REST run object's `name` correctly identified the candidate workflow
definition that executed. Goalrail therefore verifies selected-run identity via
the REST run endpoint and sabotage-tests this exact divergence. Because this
amendment changes the candidate source before tagging, the same `0.3.6` version
must receive a new candidate run before tag creation.

Critique receipt: requested and actual model `claude-fable-5`, status `success`,
no fallback, exposed cost `$0.405929`.

## Publication and rollback

Pushing a candidate branch, dispatching the workflow, creating and pushing the
tag, creating a draft GitHub Release, making that draft public, promoting
`main`, and updating the Homebrew tap each require their own exact owner
approval. Publication must use the complete verified bundle from one selected
successful run.

A build or assembly failure before tag creation publishes nothing and may be
corrected at the same version on a new commit. Once the tag exists, any source
or artifact content change requires a higher patch version; the immutable tag
and published assets are never replaced. A failed upload of the same verified
bytes may resume in the still-private draft after digest comparison. The draft
becomes public only after exact asset and digest readback passes. A defect found
after publication requires a higher patch release. The Homebrew tap remains
unchanged until its separate approved update.

## Revisit condition

Revisit when a real user needs Linux arm64, Windows arm64, an older Linux ABI
floor, package-manager ownership on Linux or Windows, Windows signing, release
attestations, multiple release operators, automated promotion, enforced tag
protection, or when runner-image drift makes the current provenance receipt
insufficient.

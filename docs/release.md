# Goalrail Release Runbook

- Status: canonical agent runbook
- Scope: native candidate verification, tag creation, GitHub Release, `main`,
  and Homebrew promotion
- Authority: every state-changing stage requires its own explicit owner approval

This file owns the release stage order. Scripts own executable validation, and
the GitHub workflow owns native builds. README descriptions are summaries, not
an alternative procedure.

## Contract

Goalrail verifies one exact remote commit on macOS arm64, Linux x86_64, and
Windows x86_64 before creating its immutable tag. The candidate workflow has
read-only repository permission and emits `READY_FOR_TAG`; it never creates a
tag, GitHub Release, branch update, or Homebrew change.

The identities carried through every stage are:

- stable version `X.Y.Z` from Cargo metadata;
- planned tag `vX.Y.Z`;
- full 40-character source commit;
- explicitly selected GitHub Actions run ID;
- run attempt, workflow name, event, and candidate branch;
- Cargo.lock SHA-256 and per-asset SHA-256 values in `release.json`.

Never infer authority for a later stage from an earlier approval. Read-only
verification may continue, but stop before each named mutation.

## 1. Freeze and verify the candidate source

Start from the exact clean commit intended for release. It must be a descendant
of the current remote `main`. Resolve identities without hand-copying them:

```sh
git fetch origin main
version="$(cargo metadata --locked --offline --no-deps --format-version 1 |
  jq -er '[.packages[] | select(.name == "gr") | .version] | unique |
    if length == 1 then .[0] else error("ambiguous gr version") end')"
source_commit="$(git rev-parse HEAD)"
tag="v$version"
candidate_branch="release-candidate/$tag"
git merge-base --is-ancestor origin/main "$source_commit"
```

Require all three local gates:

```sh
mise run ci
mise run release:candidate-check
mise run release:preflight -- "$version"
```

`release:preflight` must return `READY_FOR_CANDIDATE`, the same version and
source commit, the planned tag, no findings, and a Cargo.lock digest. It fails
when the checkout is dirty, the planned tag exists locally or on `origin`, or
the remote tag state cannot be read. Do not create a tag to satisfy preflight.

## 2. Publish only the candidate branch

Pushing the candidate branch is the first external state change. Present the
branch, source commit, version, and planned tag, then obtain approval for this
push only:

```sh
git push origin "${source_commit}:refs/heads/${candidate_branch}"
test "$(git ls-remote origin "refs/heads/$candidate_branch" | awk '{print $1}')" = \
  "$source_commit"
```

A changed remote readback is `BLOCKED`. Do not dispatch against a moving or
different branch head.

## 3. Build the native candidate

Workflow dispatch is a separate external action. After approval, pass both
identities and select the same candidate branch as the workflow definition:

```sh
gh workflow run release.yml \
  --ref "$candidate_branch" \
  -f version="$version" \
  -f source_commit="$source_commit"
```

The workflow requires its GitHub-resolved SHA, input SHA, checkout SHA, and
candidate branch head at dispatch to be identical. It also requires the planned
remote tag to be absent before and after native builds. Concurrency is keyed by
version, and a newer in-progress candidate for that version cancels the older
one.

Record one exact successful run ID. Do not select a run by artifact name alone:

```sh
gh run list --workflow release.yml --branch "$candidate_branch" --limit 10 \
  --json databaseId,headSha,status,conclusion,url,attempt,event,headBranch
run_receipt="$(scripts/check-github-run.sh \
  "$run_id" "$source_commit" "$candidate_branch" heurema/goalrail-rs)"
run_attempt="$(printf '%s\n' "$run_receipt" | jq -er '.runAttempt')"
```

Require `RUN_VERIFIED`. The checker reads the selected run through GitHub's REST
run endpoint and requires the exact repository, run ID, source commit, candidate
branch, workflow path, run name, dispatch event, positive attempt, completed
status, and successful conclusion.

Do not use `gh run view --json workflowName` as run-name evidence. GitHub CLI
resolves that field from the workflow registry on the default branch, so it may
retain the previous workflow name while a candidate branch executes a renamed
workflow. The REST run object's `name` field identifies the workflow definition
that actually ran.

## 4. Download and verify the selected bundle

Downloading is read-only. Bind the artifact to the selected run ID:

```sh
gh run download "$run_id" \
  --name "goalrail-$tag-release-candidate" \
  --dir "dist/$tag"
mise run release:bundle-check -- \
  "$version" "$source_commit" "$run_id" "$run_attempt"
jq -e \
  --arg version "$version" --arg tag "$tag" --arg commit "$source_commit" \
  --arg run_id "$run_id" --arg run_attempt "$run_attempt" '
  .version == $version
  and .tag == $tag
  and .sourceCommit == $commit
  and .run == {
    id: $run_id,
    attempt: $run_attempt,
    workflowName: "build release candidate",
    event: "workflow_dispatch",
    headBranch: ("release-candidate/" + $tag)
  }
' "dist/$tag/release.json"
```

The result is `CANDIDATE_BUNDLE_READY`, not a release. A candidate failure before
tag creation may be fixed on a new commit and rerun at the same version.

## 5. Create and push the immutable tag

Immediately before tagging, recheck that the remote tag is still absent and the
selected bundle still passes. Creating the local tag and pushing it are separate
state changes; obtain approval for each exact action.

```sh
scripts/check-remote-release-tag-absent.sh "$tag" origin
mise run release:bundle-check -- \
  "$version" "$source_commit" "$run_id" "$run_attempt"
git tag -a "$tag" "$source_commit" -m "Goalrail $tag"
git push origin "refs/tags/$tag"
scripts/check-remote-release-tag.sh "$tag" "$source_commit" origin
```

Never move, replace, or force-push the tag. After it exists, any source or asset
content change requires a higher version.

## 6. Create and verify a private draft release

Draft creation is a separate external action. Prepare exact release notes for
owner review, use the verified local files below, and never use `--clobber`:

```sh
release_dir="dist/$tag"
gh release create "$tag" \
  "$release_dir/goalrail-$tag-aarch64-apple-darwin.tar.gz" \
  "$release_dir/goalrail-$tag-aarch64-apple-darwin.tar.gz.sha256" \
  "$release_dir/goalrail-$tag-aarch64-apple-darwin.json" \
  "$release_dir/goalrail-$tag-x86_64-unknown-linux-gnu.tar.gz" \
  "$release_dir/goalrail-$tag-x86_64-unknown-linux-gnu.tar.gz.sha256" \
  "$release_dir/goalrail-$tag-x86_64-unknown-linux-gnu.json" \
  "$release_dir/goalrail-$tag-x86_64-pc-windows-msvc.tar.gz" \
  "$release_dir/goalrail-$tag-x86_64-pc-windows-msvc.tar.gz.sha256" \
  "$release_dir/goalrail-$tag-x86_64-pc-windows-msvc.json" \
  "$release_dir/goalrail.rb" \
  "$release_dir/release.json" \
  --verify-tag --draft --title "Goalrail $tag" \
  --notes-file approved-release-notes.md
mise run release:public-check -- "$version" draft "$run_id" "$run_attempt"
```

The checker revalidates the remote tag-to-source binding, requires the exact
11-asset set, and compares every GitHub-reported size and SHA-256 digest with
the verified local bundle. A failed upload may be resumed against the same
private draft only after a safe partial readback:

```sh
mise run release:public-check -- \
  "$version" draft-partial "$run_id" "$run_attempt"
```

`DRAFT_PARTIAL_VERIFIED` means every already uploaded asset is expected and has
the exact verified size and digest; its `missing` array is the only upload
allowlist. Upload only those paths, then rerun the complete `draft` check. An
extra asset or differing digest is `BLOCKED`; do not upload, delete, or replace
anything until the conflict is resolved.

## 7. Make the verified draft public

Changing draft visibility is a separate public action. After exact approval:

```sh
gh release edit "$tag" --draft=false
mise run release:public-check -- \
  "$version" published "$run_id" "$run_attempt"
```

Do not claim macOS, Linux, or Windows availability until the published digest
readback returns `PUBLISHED_VERIFIED`.

## 8. Promote the catalog and package channels

Promoting `main` is separate from publication. Recheck the tag and require a
fast-forward push of the exact source commit:

```sh
scripts/check-remote-release-tag.sh "$tag" "$source_commit" origin
git fetch origin main
git merge-base --is-ancestor origin/main "$source_commit"
git push origin "${source_commit}:refs/heads/main"
test "$(git ls-remote origin refs/heads/main | awk '{print $1}')" = \
  "$source_commit"
mise run smoke:goalrail-plugin-remote -- main "$version"
```

Homebrew promotion remains another separately approved action. Resolve the
existing tap checkout through read-only Homebrew metadata, present the exact
version, selected run ID and attempt, and tap root, then obtain approval for
this one command:

```sh
tap_root="$(HOMEBREW_NO_AUTO_UPDATE=1 brew tap-info --json heurema/tap |
  jq -er 'if length == 1 then .[0].path else error("ambiguous tap") end')"
mise run release:homebrew-promote -- \
  "$version" "$run_id" "$run_attempt" "$tap_root"
```

The promotion stage does not call `brew`. It revalidates the exact successful
candidate run, published release, bundle, public archive digest, source tag,
tap identity, GitHub write permission, and canonical SSH push transport before
editing the tap. It creates at most one formula-only commit, attempts one
non-force push, and verifies the remote Git head and formula blob afterward.
`NO_CHANGE` requires a clean tap at remote `main` and byte-identical formula;
the same version with different bytes is `CONFLICT`.

If a server rejects the push, the exact local commit is retained. Inspect it
and the remote head; do not reset, rebase, force-push, or retry automatically.
A later exact resume is another external push attempt and requires renewed
owner approval. Candidate-branch deletion is destructive cleanup and also
requires separate approval after `main`, the plugin smoke, public assets, and
Homebrew state have been verified.

## Stop and recovery rules

- Before tag creation: fix the candidate and rerun at the same version.
- After tag creation, before publication: transport or incomplete draft upload
  may resume only with the same verified bytes; content changes require a higher
  version.
- After publication: any defect or content change requires a higher version.
- Existing conflicting tag, release, asset digest, branch head, source commit,
  version, or run identity is `BLOCKED`.
- Never force-push a release tag, replace a published asset, use `--clobber`, or
  treat a successful candidate workflow as publication authority.

Checksums and manifests prove consistency among observed bytes. They are not a
signature or proof against repository, runner, account, or publisher compromise.

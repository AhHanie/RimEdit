# Nightly Builds

RimEdit publishes an automated nightly pre-release from the `development`
branch whenever it has new commits since the previous nightly.

## What a nightly is

- A GitHub **pre-release**, never promoted to "Latest".
- Built directly from a single `development` commit (shown in the release
  notes as "Source commit").
- Contains a Windows x64 installer archive and Linux x64 AppImage, `.deb`,
  and `.rpm` packages, each with a `.sha256` checksum bundled into a combined
  `SHA256SUMS.txt`.
- Not guaranteed to be stable. Nightlies exist for testing in-progress work
  between tagged releases.

Naming convention:

| Item | Format | Example |
| --- | --- | --- |
| Release title | `nightly-build-YYYYMMDD` | `nightly-build-20260815` |
| Git tag | `nightly-YYYYMMDD` | `nightly-20260815` |
| Windows archive | `rimedit-windows-x64-nightly-build-YYYYMMDD.zip` | `rimedit-windows-x64-nightly-build-20260815.zip` |
| Linux AppImage | `rimedit-linux-x64-nightly-build-YYYYMMDD.AppImage` | `rimedit-linux-x64-nightly-build-20260815.AppImage` |
| Linux Debian package | `rimedit-linux-x64-nightly-build-YYYYMMDD.deb` | `rimedit-linux-x64-nightly-build-20260815.deb` |
| Linux RPM package | `rimedit-linux-x64-nightly-build-YYYYMMDD.rpm` | `rimedit-linux-x64-nightly-build-20260815.rpm` |

## Where it runs

The workflow lives at `.github/workflows/nightly.yml` and runs:

- On a schedule, once nightly at 01:15 UTC.
- On demand via **Actions -> Nightly Builds -> Run workflow**, with an
  optional `force` input.

It always checks out the `development` branch explicitly (not whatever
branch triggered the run) and records the exact commit SHA it built before
starting any build job, so every nightly is reproducible from its release
notes.

### Why it might not publish anything

The workflow only builds when `development` has commits that are not in the
most recent existing `nightly-*` release. A scheduled run with no new
commits completes successfully without creating a tag, release, or assets --
this is expected, not a failure.

## The `force` input

Use `force=true` from **Run workflow** for two situations:

1. **Retrying a failed nightly.** If a build job failed after a release
   *would* have been created for today's date, but before `publish` actually
   ran, no tag exists yet and a plain re-run (no `force` needed) will pick
   the same commit back up. If a release for today's tag was *already*
   created and you need to replace its assets, `force=true` allows the retry
   as long as `development` has not moved past the commit that release
   already targets. It will refuse to retarget an existing tag to a
   different commit.
2. **Bootstrapping after a history change.** The workflow compares the
   previous nightly's commit against the current `development` HEAD with
   `git merge-base --is-ancestor`. If the previous nightly's commit is *not*
   an ancestor of the new HEAD -- for example after a rebase, a force-push,
   or (as with this repository's initial rollout) because older `nightly-*`
   tags were created by hand against merge commits on `main` instead of
   commits directly on `development` -- the workflow fails safely instead of
   guessing at a commit range. Re-run manually with `force=true` to build
   unconditionally once you've confirmed that's expected.

## Recovery

- **A build job failed:** re-run the workflow (manually, or wait for the
  next schedule). No release was created, so this is a normal retry.
- **The publish job failed after builds succeeded:** re-run manually with
  `force=true`; it will reuse today's tag only if it still points at the
  same `development` commit.
- **You need to skip a broken commit:** just wait for the next `development`
  commit and the next scheduled run; nightlies are commit-addressed, not
  required to run every day.

## Local testing

You can validate a build locally with the same commands the workflow runs:

```
pnpm install --frozen-lockfile
pnpm tauri build -- --bundles nsis            # Windows
pnpm tauri build -- --bundles appimage,deb,rpm  # Linux
```

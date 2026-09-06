# Plannotator fork

The clean baseline is upstream **v0.27.12**, commit
`96313ab228ede843203d38d9d2a86e1c87e18c81`, on
`Yassimba/plannotator:loom/v0.27.12`, preserved as the baseline branch.
The default branch, `loom/current`, carries the fork's CI and sync automation
plus the figure-led Guided Review patches (`sections[].diagrams` as inline
SVG with `data-code` bindings, the document layout with a code peek, and
`plannotator guide import`). `origin` is the fork and `upstream` is
`backnotprop/plannotator`.

Loom installs the fork's release (`v0.27.12-loom.N`) through
`manifest/loom.toml`; `cli/loom/src/manifest.rs` moves a selection made
under the upstream key back to the fork. Keep using upstream's Claude
plugin. Cut a fork release only when `loom/current` changes runtime
behavior: tag `v<upstream>-loom.N` on `loom/current`, compile the six
binaries with `bun build --compile` per target as `release.yml` does, and
create the GitHub release by hand (upstream's release workflow refuses tags
that are not on `main`).

## Small changes

Start each change from `loom/current`. Keep one behavior change
per commit, with a focused regression check. Prefer upstream's existing
configuration and extension points. Send generally useful fixes upstream;
drop patches once upstream contains them. Keep Loom-specific agent guidance
in Loom's skills rather than changing Plannotator's application prompts.

Do not merge the old fork's `main` into `loom/current`: its Guided Review
work was rebased onto v0.27.12 and merged through PR #2 instead. Leave upstream version files, lockfiles,
and publishing workflows alone unless the change actually requires them.

## Automatic upstream updates

The fork's `Sync upstream release` workflow runs daily at 06:23 UTC and can
also be run manually in GitHub Actions. It opens one `upstream-sync/v…` PR
at a time against `loom/current`, using the latest stable upstream release.
It merges upstream history into our branch, preserving our patches and Git's
knowledge of what has already been integrated. No force pushes or squashes.

The required `Loom checks` job runs upstream's release-version check, typecheck,
test suite with Bun's file isolation, and production review UI, plan UI, and
standalone CLI build. It also exercises the sync script against temporary Git
repositories. Branch
protection requires an up-to-date branch and passing checks, including for
administrators. Native auto-merge uses a merge commit after those checks pass.

Conflicts produce an open PR with auto-merge off. Resolve them in its branch;
the next scheduled run updates the branch, dispatches checks, and enables
auto-merge. Failed checks keep the PR open. Closing a PR without merging it
declines that release; the bot does not reopen it. An unresolved sync PR blocks
newer releases until it is handled.

The workflow uses the built-in GitHub token. It explicitly dispatches checks
because PR events created with that token can require human workflow approval.
No personal token or additional secret is needed. The sync job can write to
GitHub but never runs upstream application code; checks run with a read-only
token. Repository settings allow Actions to create PRs and enable auto-merge.

This updates the fork, not installed binaries. Loom's Renovate config updates
the upstream binary pin separately, with its existing five-day release delay.
Update the submodule pointer in Loom when adopting fork changes. With no
runtime patches, keep the upstream pin in `manifest/loom.toml`.
With runtime patches, first build and publish a
fork release from the reviewed branch, then change the binary source and pin
in Loom and update its tool-key migration. Never point the installer at a
release before its platform assets exist. Regenerate the setup catalog and
run `npm run check` before shipping Loom.

## Preserved work (2026-09-05)

These local branches in `cli/plannotator` preserve the previous work:

- `archive/loom-main-2026-09-05`: published fork main at `7ed8c95f`.
- `archive/anchored-diagrams-2026-09-05`: local SVG work at `2065c10c`.
- `archive/svg-wip-2026-09-05`: full snapshot of the six uncommitted SVG/UI
  files, also retained in the stash named “Before clean upstream v0.27.12
  baseline (2026-09-05)”. The branch protects the snapshot even if the stash
  list changes. To resume with the original index and working changes, switch
  to `feature/anchored-diagrams`, then run
  `git stash apply --index archive/svg-wip-2026-09-05`.

Existing remote branches and releases remain available. Archive branches and
the stash snapshot are local backups, not published branches.

The old repository SVG directive and custom in-app changeset-walkthrough
integration are not carried forward. The current Blueprint SVG instructions
still describe the archived experiment; do not assume upstream renders those
directives. Review standalone HTML with `plannotator annotate` until those
instructions are revised or a small replacement is explicitly selected.

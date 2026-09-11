---
name: glab
description: Use when creating, reading, updating, or closing GitLab issues and merge requests with the glab CLI.
version: "0.1.0"
metadata:
  platform: gitlab
---

# GitLab CLI

Use `glab` for GitLab issue and merge request operations. Run commands from the repository root so `glab` can infer the project.

## Merge requests

Create one feature branch and one commit before opening its merge request.

```bash
git switch -c feat/short-description
git add <files>
git commit -m "feat: short description"
git push -u origin feat/short-description
```

Create the merge request with an explicit target branch and self-assignment:

```bash
glab mr create \
  --title "feat: short description" \
  --description "$(cat <<'EOF'
## Summary
- What changed and why
EOF
)" \
  --target-branch main \
  --assignee @me
```

Use branch names:

- `feat/short-desc`
- `fix/short-desc`
- `chore/short-desc`

Use conventional commit prefixes in MR titles: `feat:`, `fix:`, `chore:`, `refactor:`, or `docs:`.

Use `--draft` for draft MRs. Never force-push to `main`.

Read or inspect MRs:

```bash
glab mr list
glab mr view <mr-number> --comments
glab mr diff <mr-number>
```

## Issues

Create and inspect issues:

```bash
glab issue create --title "Short title" --description "Description"
glab issue view <issue-number> --comments
glab issue list -F json
```

Use `glab issue note <issue-number> --message "..."` for comments. Apply labels with `glab issue update <issue-number> --label "label"`; remove labels with `--unlabel`. Close an issue with `glab issue close <issue-number>`.

## Authentication

Check authentication before write operations:

```bash
glab auth status
```

If authentication fails, stop. Keep tokens out of commands and output.

## Verification

Before opening an MR:

```bash
git status --short --branch
```

After opening an MR, record its URL and inspect its pipeline status with `glab mr view <mr-number>`.

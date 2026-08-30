---
name: check-for-updates
description: Read-only briefing of stale loom TUI tools and remote skills, with merge class against local edits.
disable-model-invocation: true
---

# Check for updates

This run is a **briefing**. Apply starts on a later turn, when the user names an item (`npx skills update <name>`, or `loom update` then mise for a TUI).

Write the briefing in the write-simply **register**.

## 1. Inventory

Build two lists. Done when every selected TUI and every remote skill is on a list, each with a name, a pin or lock, and a bin or live path.

**TUI** — selected in `~/.config/mise/conf.d/loom.toml`, and the matching `manifest/tools.json` description names TUI or Terminal UI. Pin = the SELECTION line. Bin = the row's `bin` (or `label`).

**Remote skill** — entries in `~/.agents/.skill-lock.json` and, when present, the project's `skills-lock.json` whose `sourceType` is `github`, `git`, `gitlab`, or `well-known`. Live path = `npx skills ls -g --json` (and `npx skills ls --json` in the project), following the symlink to the real folder.

## 2. Detect

Label each inventory item **current** or **stale**. Done when every item has one label.

**TUI** — GitHub pin: `gh api repos/<owner>/<repo>/releases/latest` (tags when the pin is a tag). npm pin: `npm view <pkg> version`. **stale** when latest ≠ pin.

**Remote skill** — lock field is `skillFolderHash`.

- GitHub: `GET /repos/<source>/git/trees/<ref>?recursive=1` (`ref`, or the default branch when `ref` is empty). Folder SHA = the `tree` entry for `dirname(skillPath)`, or the root tree SHA when the skill sits at repo root.
- git / gitlab: `ls-tree` the skill folder at that ref.
- well-known: GET `sourceUrl` and digest the payload.

**stale** when that SHA or digest ≠ `skillFolderHash`. Clone when the API is down.

## 3. Brief each stale item

Done when every stale item has a **brief** and a **merge**.

**brief** (register):

- What it does, one sentence from the catalog row or SKILL.md description.
- What moved since the pin or lock: the release, commits, or SKILL.md change, named specifically.

**merge** — live vs lock vs upstream:

| Class       | Meaning                                                                                                                          |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------- |
| **clean**   | live = lock; only upstream moved. Apply is a straight take.                                                                      |
| **drift**   | live ≠ lock; upstream = lock. Apply overwrites local edits.                                                                      |
| **overlap** | live and upstream both ≠ lock on the same region. Name each file and the colliding hunk (local edit and upstream edit).          |

Fetch the upstream skill folder into a temp dir (GitHub contents/raw, or the clone from detect). `diff -ru` live vs upstream.

- live folder still matches the lock hash → every diff is **clean**
- live folder no longer matches the lock hash → hunks both sides moved are **overlap**; only live moved, **drift**; only upstream moved, **clean**

Item class is the worst hunk: overlap > drift > clean.

TUI binaries have no skill tree: **clean**, unless the user keeps a local fork (say so).

## 4. Overview

One document: TUI, then remote skills.

- **current**: one count line
- **stale**: name, current → latest, brief, merge class, and merge detail when the class is not clean
- Close with a **pick list**: the stale names

Done when every inventory item is in the document and the pick list is last.

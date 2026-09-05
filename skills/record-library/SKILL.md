---
name: record-library
description: Record a library in the refactor catalog — the package plus the hand-roll it retires — so future OSS reviews hunt with it.
disable-model-invocation: true
---

# Record library

Record one or more libraries in the `refactor` library catalog, so every future OSS review checks whether they apply.

## 1. Locate the catalog

Find `references/oss-libraries.md` inside the installed `refactor` skill. Check the directory beside this skill first, then glob the agent's skill trees for `refactor/references/oss-libraries.md`. When no catalog turns up, report that `refactor` is not installed and stop.

Done when exactly one catalog file is open.

## 2. Shape the entry

An entry is one line in the catalog's own format: the package name, then the hand-roll it retires. Take the package from what the user said; take the "hand-roll it retires" half from the user's words, from the surrounding conversation, or — when neither says — from a quick look at the package's documentation. When the user gives only a name and the retired hand-roll stays unclear after the docs lookup, ask.

Done when each entry states a package and the specific custom code it replaces — "useful library" is not an entry.

## 3. Append

Place each entry in the ecosystem section it belongs to, creating the section when the ecosystem is new to the catalog. When the package is already listed, sharpen the existing line with the new information instead of adding a second one.

Done when the catalog contains each entry exactly once, in section, in format.

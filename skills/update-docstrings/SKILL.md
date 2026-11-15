---
name: update-docstrings
description: Add or update docstrings, doc comments, and inline comments in source code — any language. Use when the user wants docstrings written or fixed, a conversion to a doc style (NumPy, TSDoc, rustdoc), or a comment cleanup. Source files only — README/ADR/prose docs belong to write-documentation / write-readme.
---

# Update Docstrings

The reader has the source file and nothing else. They have little attention. Write for them.

Change docstrings and comments only. When the real fix is code, propose it.

## Rules

- **Now only.** Say what the code does and how. No history, no plans, no "could", no ticket or ADR names.
- **Short.** One summary line. Then at most one short paragraph and one sketch.
- **Plain words.** Write for a user of the tool. Replace a jargon word with the plain word. Gloss a project word only when no plain word exists.
- **Draw it.** For a flow, a before and after, a timeline, or a decision, add one ASCII sketch of at most 10 lines. The sketch replaces prose. It does not repeat it.
- **Real values.** Examples use values from the codebase. Name a constant, do not copy its value.
- **Every symbol gets one.** A private symbol gets one sentence. A public symbol gets the sections of its dialect, where each section says what the signature does not.
- **The code wins.** When the docstring and the body disagree, rewrite the docstring.
- **Keep `@cc` contract lines** word for word.

## A sketch

```
value below zero? ---- no ----> None
        |
       yes
        v
time unit? ----------- no ----> None
```

## Comments

A comment says why, never what. Delete restatements, commented-out code, banners, and doubts.

## Steps

1. Invoke the `write-simply` skill.
2. Read the language reference: [Python](references/python.md), [TypeScript](references/typescript.md), [Rust](references/rust.md). For another language, use its main convention.
3. Read each body, then write its docstring.
4. Run the doc linter that the reference names. Fix each finding.

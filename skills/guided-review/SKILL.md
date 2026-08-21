---
name: guided-review
description: Guided code review that walks the human through a diff in small, semantically ordered chunks, each approved individually, until they can vouch for every line. Use when an agent produced more change than the user has followed, when the user must review a PR or branch they didn't write, or when they ask to be walked through a diff.
---

# Guided Review

The common joke: a 5-line PR gets 10 comments; a 5000-line PR gets "LGTM" and merges. This skill exists to prevent the LGTM merge. The human wants to understand and vouch for every technical choice, architectural choice, line of code, style choice, layout, degree of abstraction, and pattern used — and nobody gets there by reading an alphabetically ordered file-tree diff. Get there through small chunks, in an order that builds shared understanding, each individually approved.

## Ground rules

- Invoke the `write-simply` skill (via the Skill tool) — every chunk message follows its register.
- Use 100× less text than you normally would. Simple is king — and when a chunk can't be explained simply, treat that as a finding: the code may be overly complex, and a simpler solution is _actually_ the smarter option. (Code that looks stupidly easy is incredibly difficult to write.)
- Code is an unfortunate means to an end. If the reviewer doesn't know the end, the guided review failed its job — open with the end, not the code.
- A tiny visualization says a thousand words: call-site sketches, before/after trees, touch-point maps in small ASCII. A wall of text overloads the reviewer.
- The user approves every chunk by replying manually in chat. Never call AskUserQuestion — present the chunk, stop, and wait for their message. Approval advances to the next chunk; a question or objection reopens the current one.
- This is a real review, not a tour — we all make mistakes, so suggest improvements as you go. Time is not an issue; correctness, readability, and long-term maintainability are. Flag especially:
  - Two options with identical inputs and outputs where one has a branch fewer — prefer it even when it looks slightly more complex (Linus: the reduced branching also reduces cognitive load).
  - Moves toward the pit of success — e.g. strongly typed IDs, so the type checker yells when a `ProductID` is passed where a `CartItemID` is expected.

## Flow

### 1. Scope

Determine the diff: a PR (`gh pr diff`), a branch against its merge target, or the working tree. Record the total churn (`git diff --stat`) — the context chunk states it, and every later chunk reports its share of it.

### 2. Context chunk

The why/what before any code — the reviewer may have pasted a PR link cold, or picked up a colleague's WIP branch after they left on holiday:

- The business or user-facing reason the change exists ("an email from the CEO: an additional 10% charge on every line item — and our current architecture fights this need, so we restructure")
- What the diff accomplishes, in one or two sentences
- Total churn: `+520 −310 across 14 files`

### 3. Chunk plan

Split the entire diff into semantically ordered chunks — the order that builds understanding, never file order. A typical arc:

1. Baseline and options: given the current structure, options (A), (B), (C) — this PR uses (B) because … (the reviewer might suggest otherwise; that's the point)
2. The failing test that drove the change, with its exact message
3. Whether existing tests already covered the area being refactored
4. The new core pattern
5. Adoption at each call site
6. The cleanup the change made possible

The plan is complete only when every changed line lands in exactly one chunk — the per-chunk churn must sum to the total.

### 4. Walk the chunks

One message per chunk:

- What this chunk is for, in a sentence
- The diff — the load-bearing lines shown and called out, the mechanical rest summarized
- Churn: `(+50 −15 | 5% of the PR)`
- Key decisions worth the reviewer's attention
- Improvement suggestions, when you have them

Then stop and wait for the manual reply.

### 5. Close

The review is complete when every chunk is approved. Recap the suggestions the user accepted so they can become follow-up work — the user can now give a verdict they can honestly vouch for, because they've seen every line.

## User context

The user may have added extra info they think would be useful:

<user_context>

$ARGUMENTS

</user_context>

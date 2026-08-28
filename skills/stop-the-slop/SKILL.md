---
name: stop-the-slop
description: Remove AI writing patterns from a draft, in Preserve stance (keep the writer's voice) or Improve stance (rewrite toward the best document, facts sourced from the repo). Detects patterns without rewriting, or scores, on request. Use when text should read less AI-written, or when asked whether it does.
---

# Stop the slop

You are a sharp human editor. Remove AI patterns while keeping the writing alive. Distinctive writing stays distinctive; reference prose gets every fact it should carry.

Load `writing-clearly-and-concisely` first: it sets the register (Simplified Technical English plus Zinsser) every edit is written in. This skill adds the AI-pattern removal and the stance on top.

## Stance

Pick the stance first. Every later rule bends to it.

**Preserve.** Personal writing: blog posts, newsletters, talks, anything with an "I". Keep the writer's point, structure, and voice. Make the minimum effective edit. Keep only facts the draft already had.

**Improve.** Reference docs: READMEs, API docs, runbooks, changelogs, copy with no author voice. Rewrite toward the best document: fill gaps from the codebase or linked sources, restore detail the draft blurred, drop sections that carry nothing. Every added fact needs a source you read.

Signals: first person, humor, digressions, opinions mean Preserve. No "I", generic structure, facts verifiable in the repo mean Improve. The user can say `preserve` or `improve` up front; when the draft is mixed and the user said neither, ask: "Keep this as your voice, or rewrite toward the best version of the doc?"

## Jobs

**Edit (default).** Return the edited draft plus a **What changed** section that opens with the stance used.

**Detect.** The user asks whether a piece is AI slop, or to audit, scan, or flag without rewriting. For each pattern found: name it, quote the line, give the fix in a few words. Named patterns are evidence the user can check; AI detectors guess, so skip the guess about authorship. Offer to edit after.

**Score.** Only when the user asks for a number. Rate 1-10 per dimension, show the table, name the two lowest with one quoted line each. Below 35/50: recommend an edit.

| Dimension | Question |
|-----------|----------|
| Directness | Statements or announcements? |
| Rhythm | Varied or metronomic? |
| Trust | Respects reader intelligence? |
| Authenticity | Sounds like a person? |
| Density | Anything cuttable? |

## Before editing

No draft: ask for it. Audience or format unclear: ask who it is for and where it will be published. Goal unclear: ask what the reader should think, feel, or do afterwards.

## Editing principles

Both stances:

- **Lead with the point when the setup adds nothing.** Keep a personal aside, story, or admission when it creates context, tension, or character.
- **Open it up without dumbing it down.** Keep substance, nuance, precision. Strip jargon, long sentences, abstract nouns, tangled structure.
- **Active voice, human subjects.** "The team shipped it Tuesday" beats "the decision emerged." Name the person doing the verb.
- **Concrete over abstract.** "The integration cut deploy time from 40 minutes to 4" beats "improved efficiency." Protect the specific fact; a number the draft had stays a number.
- **Direct verbs.** "Decided" over "made a decision," "can" over "has the ability to."
- **Keep hedges that carry meaning.** "I think," "maybe," "to be honest" stay when they express real uncertainty or the writer's spoken rhythm.

Preserve only:

- Notice the draft's vocabulary, cadence, bluntness, humor, uncertainty, digressions, and level of polish first. Keep the traits that feel personal. Leave strong human sentences alone; a rough draft with a real voice still sounds like the same person afterwards.
- Keep strong opinions, blunt language, profanity, self-interruptions, honest admissions. Keep longer spoken sentences, fragments, and changes of pace when they are clear.
- Keep the structure and detours unless they hurt the piece; if you reorganize, say why in What changed.

Improve only:

- Read the sources the draft points at (the repo, linked docs) and carry the facts a reader of this document needs. A vague claim the source can sharpen ("non-destructive" → "link only replaces directories proven identical") gets sharpened.
- Restructure freely toward the clearest document; the draft's shape is a suggestion.

## Words to cut

Banned outright: delve, foster, leverage, utilize, facilitate, empower, streamline, robust, cutting-edge, paradigm shift, game changer, this is huge, this changes everything, tapestry, realm, beacon, multifaceted, meticulous, intricate, paramount, transformative, elevate, embark, supercharge, harness, ever-evolving. Long-tail list: [references/phrases.md](references/phrases.md).

Often-empty adverbs (just, literally, honestly, simply, actually, truly, fundamentally, importantly, crucially, inherently, inevitably) and phrases (it's worth noting, at the end of the day, when it comes to, at its core, in today's world, the reality is, in terms of, in order to, going forward, let's dive in): cut when they delay the point, keep when they carry emphasis, contrast, or the writer's spoken rhythm.

## Patterns to cut

Seventeen structural patterns, each with a before/after, live in [references/patterns.md](references/patterns.md): binary contrasts, throat-clearing openers, faux-insight setups, colon reveals, superficial `-ing` analysis, importance puffery, weasel attribution, fake-strong verbs, synonym cycling, negative listing, dramatic fragmentation, robotic rhythm, rhetorical setups, fake-profound kickers, summary-recap endings, formatting slop, em dashes. Read it before the first edit; more examples in [references/structures.md](references/structures.md).

## Workflow

1. Read the full draft. Pick the stance. Identify the core point and, in Preserve, 3-5 voice signals to keep (internal note). Core point unclear: ask.
2. Detect request: return the findings report from Jobs and stop.
3. Edit: apply the principles and patterns for the stance in the `writing-clearly-and-concisely` register, then check the result against `eval.md`. Any fail: fix and re-check.
4. Output the full edited draft and What changed, opening with the stance.

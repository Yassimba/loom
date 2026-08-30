---
name: stop-the-slop
description: Remove AI writing patterns from a draft, in Preserve stance (keep the writer's voice) or Improve stance (rewrite toward the best document, facts sourced from the repo). Detects patterns without rewriting, or scores, on request. Use when text should read less AI-written, or when asked whether it does.
---

# Stop the slop

You are a sharp human editor. Remove AI patterns while keeping the writing alive. Distinctive writing stays distinctive; reference prose gets every fact it should carry.

Load `write-simply` and use its **register** section; nothing further from it unless the Jobs below name a file. The register (Simplified Technical English plus Zinsser) is the voice every Improve edit is written in and the floor every Preserve edit is measured against. This skill adds the stance and the slop catalogue on top.

## Stance

Pick the stance first. Every later rule bends to it.

**Preserve.** Personal writing: blog posts, newsletters, talks, anything with an "I". Keep the writer's point, structure, and voice. Make the minimum effective edit. Keep only facts the draft already had. Where the register and the voice collide (a 40-word spoken sentence, a fragment, a hedge), the voice wins; the register applies to sentences that were slop anyway.

**Improve.** Reference docs: READMEs, API docs, runbooks, changelogs, copy with no author voice. Rewrite toward the best document in the register: fill gaps from the codebase or linked sources, restore detail the draft blurred, drop sections that carry nothing. Every added fact needs a source you read.

Signals: first person, humor, digressions, opinions mean Preserve. No "I", generic structure, facts verifiable in the repo mean Improve. The user can say `preserve` or `improve` up front; when the draft is mixed and the user said neither, ask: "Keep this as your voice, or rewrite toward the best version of the doc?"

## Jobs

**Edit (default).** Return the edited draft plus a **What changed** section that opens with the stance used.

**Detect.** The user asks whether a piece is AI slop, or to audit, scan, or flag without rewriting. Load `write-simply/references/signs-of-ai-writing.md` as the catalogue alongside this skill's. For each pattern found: name it, quote the line, give the fix in a few words. Named patterns are evidence the user can check; AI detectors guess, so skip the guess about authorship. Offer to edit after.

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

The register already covers active voice, concrete words, direct verbs, and needless words. On top of it, both stances:

- **Lead with the point when the setup adds nothing.** Keep a personal aside, story, or admission when it creates context, tension, or character.
- **Name the person doing the verb.** "The team shipped it Tuesday," where the draft had "the decision emerged."
- **Protect the specific fact.** A number, name, or mechanism the draft had stays; the register's "omit needless words" applies to words, never to facts.
- **Keep hedges that carry meaning.** "I think," "maybe," "to be honest" stay when they express real uncertainty or the writer's spoken rhythm.

Preserve only:

- Notice the draft's vocabulary, cadence, bluntness, humor, uncertainty, digressions, and level of polish first. Keep the traits that feel personal. Leave strong human sentences alone; a rough draft with a real voice still sounds like the same person afterwards.
- Keep strong opinions, blunt language, profanity, self-interruptions, honest admissions. Keep longer spoken sentences, fragments, and changes of pace when they are clear.
- Keep the structure and detours unless they hurt the piece; if you reorganize, say why in What changed.

Improve only:

- Read the sources the draft points at (the repo, linked docs) and carry every fact a reader of this document needs; done means a reader can act on the doc without opening those sources. A vague claim the source can sharpen ("non-destructive" → "link only replaces directories proven identical") gets sharpened.
- Restructure freely toward the clearest document; the draft's shape is a suggestion.

## Words to cut

Banned outright: delve, foster, leverage, utilize, facilitate, empower, streamline, robust, cutting-edge, paradigm shift, game changer, this is huge, this changes everything, tapestry, realm, beacon, multifaceted, meticulous, intricate, paramount, transformative, elevate, embark, supercharge, harness, ever-evolving. Long-tail list: [references/phrases.md](references/phrases.md).

Often-empty adverbs (just, literally, honestly, simply, actually, truly, fundamentally, importantly, crucially, inherently, inevitably) and phrases (it's worth noting, at the end of the day, when it comes to, at its core, in today's world, the reality is, in terms of, in order to, going forward, let's dive in): cut when they delay the point, keep when they carry emphasis, contrast, or the writer's spoken rhythm.

## Patterns to cut

Seventeen structural patterns, each with a before/after, live in [references/patterns.md](references/patterns.md): binary contrasts, throat-clearing openers, faux-insight setups, colon reveals, superficial `-ing` analysis, importance puffery, weasel attribution, fake-strong verbs, synonym cycling, negative listing, dramatic fragmentation, robotic rhythm, rhetorical setups, fake-profound kickers, summary-recap endings, formatting slop, em dashes. Read it before the first edit; more examples in [references/structures.md](references/structures.md).

## Workflow

1. Read the full draft. Pick the stance. Identify the core point and, in Preserve, 3-5 voice signals to keep (internal note). Core point unclear: ask.
2. Detect request: return the findings report from Jobs and stop.
3. Edit: apply the principles and patterns for the stance, then check every sentence against `eval.md`. Any fail: fix and re-check until all pass.
4. Output the full edited draft and What changed, opening with the stance.

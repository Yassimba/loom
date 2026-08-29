---
name: explain-simply
description: Explain something new fast for a smart adult who has never used the topic. Use for "/explain-simply [topic]", "explain this simply", "eli5 this", "break this down for me", "I know nothing about X, catch me up", or another request for a quick plain-language explanation of how something works.
---

# ELI5

Get the user the gist of an unfamiliar topic in ten seconds, then explain it in a few clean beats. Treat "5" as a stand-in, not an age: the reader is an intelligent adult who knows nothing about this topic and everything else about the world.

- Do not explain ordinary adult concepts such as money, the internet, a company, a manager, an admin, a customer, a phone, or a file.
- Do not talk down to the reader.
- Keep the answer tight and conversational. Do not write a wall of text.

## Calibrate and answer

State the knowledge boundary in one sentence, then answer without waiting for a reply:

> Assuming you know what a server is but not what a webhook does — tell me if I am off.

Never open with questions or an introductory preamble.

## Verbal walkthrough

Use this order:

1. Orient the reader in one line. Say where the topic lives and what it is about.
2. Give the core in one plain sentence. This sentence must carry the point if the reader stops there.
3. Say "Here's how it works:" and walk through the main process in about five numbered steps. Each step is one or two natural sentences.
4. Teach each key term in place, in **bold**, at the moment it happens. Do not define terms before the story needs them.
5. End with one closing truth that makes the point, safety, or consequence clear.

Prefer specific and literal language. Keep the real jargon when the reader needs it to search or discuss the topic, and define it briefly on first use. Use one adult analogy only when plain words do not explain the idea well. Never use childlike analogies.

Do not front-load every aspect of the topic. Follow the main path; depth belongs in follow-up questions.

## Rendered story graphic

After the verbal walkthrough, add one rendered graphic when the topic has a flow, sequence, before-and-after change, or one thing acting on another. Skip the graphic for static concepts with no useful motion, such as a variable or open source.

Never use a Mermaid block or another code-block diagram as the graphic. Build one self-contained HTML file with inline SVG and deliver it as a rendered artifact.

Make the graphic a vertical comic strip:

- Use separate, stacked scenes. Each scene shows one left-to-right action with three or four visual elements.
- Write each panel title as a plain subject-verb-object sentence. Reading the titles from top to bottom must tell the full story.
- Reuse the same recognizable SVG characters in every panel. Use friendly people, documents, robots, browser windows, or other concrete objects instead of changing abstract rectangles.
- Put one plain caption under each scene. The caption adds why the step matters, a concrete example, or a caveat.
- Split any scene that contains two actions.

The subhead or caption must add information. It must not restate the panel title.

### Visual style

Use this light editorial style:

- Set the page background to `#F7F8FC`.
- Use Georgia at weight 400 for the title question and panel titles.
- Use Helvetica or Arial for body text, captions, and labels.
- Use `#E7EAF6` and `#DDE2F2` for lavender bands and fills, `#111111` for ink, `#5F6272` for secondary text, and `#C42A1C` as the only accent.
- Use the accent sparingly for step labels, the active or user-owned element, key terms, or one arrow. Do not use purple or green.
- Use near-square corners with a 3–4 px radius and `1.5px solid #111111` card borders.
- Use a full-width lavender hero band with a `1.5px` black bottom rule.
- Use small uppercase step labels at about 11 px with `letter-spacing: .12em` in brick red.

Do not add meta copy such as `/explain-simply`, category labels, claims about the reader, or a decorative footer. The page is the explanation.

Before delivery, check two things:

1. The panel titles alone tell the whole story.
2. Each picture is clear in two seconds without tracing a path.

## Hand-off

For a topic with a graphic, end the verbal answer with exactly this low-pressure line, then attach the rendered file in the same turn:

> Here's a quick graphic in case helpful:

For a static topic without a graphic, close with one plain offer to go deeper. Do not add a summary or a list of possible next topics.

## Follow-ups

Stay in this mode for follow-up questions. Use what the reader has shown they know, skip covered ground, and go one level deeper.

## Never do this

- Open with praise or a preamble such as "great question" or "let's get you up to speed."
- Say "simply put," "it's easy," or compare the reader to a child.
- Stack analogies or dense sections.
- Restate what the reader said they know.
- Define ordinary adult-life words.
- Produce a wall of text.

Topic: $ARGUMENTS

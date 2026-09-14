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

## Mermaid story graphic

After the verbal walkthrough, add one Mermaid diagram.

Use a fenced `mermaid` block so the diagram renders in the conversation.

- Keep the main path to about five steps.
- Write each node or message as a plain subject-verb-object sentence.
- Reading the labels in order must tell the full story.
- Put caveats or examples on edges only when they add information.
- Split any node that contains two actions.

Do not add meta copy such as `/explain-simply`, category labels, or claims about the reader. The diagram is the explanation.

Before delivery, check two things:

1. The labels alone tell the whole story.
2. The path is clear in two seconds without tracing crossed lines.

## Hand-off

For a topic with a diagram, end the verbal answer with exactly this low-pressure line, then include the Mermaid block in the same turn:

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

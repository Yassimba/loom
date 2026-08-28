---
name: writing-clearly-and-concisely
description: Writing or editing prose humans read - docs, commit messages, error messages, reports, UI text. Also the register other skills invoke for compact technical prose (docstrings, comments, captions, proposals), and the pass that makes text sound human instead of AI-written ("humanize", "reads like ChatGPT").
---

# Writing Clearly and Concisely

Always use ASD-STE100 Simplified Technical English plus Zinsser's four principles
Write vigorous prose: every word tells, sentences are active and concrete, needless words are gone. Say what the thing does, in the words a knowledgeable human would pick.

## Principles

Strunk's principles of composition, applied to everything you write:

- One paragraph per topic; open it with a topic sentence.
- Use the active voice.
- Put statements in positive form.
- Use definite, specific, concrete language.
- Omit needless words.
- Express co-ordinate ideas in similar form; keep related words together.
- Place emphatic words at the end of the sentence.

## Register for compact prose

Docstrings, comments, commit messages, captions, a numbered list of proposals: ASD-STE100 Simplified Technical English plus Zinsser's four principles — clarity, simplicity, brevity, humanity. On top of the principles above:

- One idea per sentence; about 20 words or fewer.
- One meaning per word; the same word for the same thing every time.
- Simple present tense; instructions as imperatives.
- Plain words; name the specific thing.
- One person talking to another.

A skill that says "use the register" means this section; nothing further to load.

## What to load

Match the task to one row and load exactly that file:

| Task                                                 | Load                                                                        |
| ---------------------------------------------------- | --------------------------------------------------------------------------- |
| Writing or editing paragraphs, docs, explanations    | `references/elements-of-style/03-elementary-principles-of-composition.md`   |
| Fixing grammar, commas, punctuation in existing text | `references/elements-of-style/02-elementary-rules-of-usage.md`              |
| Choosing the right word, fixing common misuses       | `references/elements-of-style/05-words-and-expressions-commonly-misused.md` |
| Headings, quotations, formatting                     | `references/elements-of-style/04-a-few-matters-of-form.md`                  |
| load this always                                     | `references/humanizer.md`                                                   |
| Spotting AI patterns while writing (words to watch)  | `references/ai-patterns.md`                                                 |
| Auditing whether text is AI-written (full catalogue) | `references/signs-of-ai-writing.md`                                         |

**Full pass** — load `02`, `03`, `04`, `05`, `humanizer.md`, and `ai-patterns.md` together — only when the user asks for a substantial new piece ("write the docs", "draft the README") or names the depth ("full pass", "thorough edit", "load everything"). "Fix this paragraph" stays with one file.

## Done means

Re-read the full output. It is done when every sentence passes:

- each word informs;
- the active voice wherever it works;
- a specific where a generic stood;
- the human's word where an AI pattern stood (puffery, promotional adjectives, "delve"/"leverage", decorative formatting).

A failed sentence is fixed, then the whole output is re-read again.

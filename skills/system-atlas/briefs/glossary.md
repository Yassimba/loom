# Glossary brief

Write a plain-language glossary for readers new to <PRODUCT>. Voice: Simplified Technical English, one idea per sentence, no em dashes, no parentheses, no jargon inside a definition unless it is itself defined. Each meaning is 1 to 3 sentences, 15 to 45 words, and says what the thing IS and what <PRODUCT> DOES with it.

Sources: the notes in <WORKDIR>/notes/*.md and, when needed, the code.

Cover 45 to 60 terms in these groups, in this order, with a "group" field on each: 1 the product's domain concepts; 2 the organisation's data sources and systems; 3 cloud and tooling; 4 the product itself, with one line per repository and its main run modes. The orchestrator gives you a TERMS list to start from. Add terms the notes use that a newcomer would not know.

Output: JSON at <WORKDIR>/glossary.json with shape {"intro": "2-3 sentences on how to use this glossary", "terms": [{"group": "...", "term": "...", "meaning": "..."}]} in group order. Reply with only the term count.

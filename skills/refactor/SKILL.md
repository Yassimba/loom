---
name: refactor
description: Refactor code through five parallel reviews—ecosystem reuse, design patterns, tests, types.
disable-model-invocation: true
---

# Refactor

Refactor the scope given by the user, if no scope is given default to the uncomitted changes, if there are no uncomitted changes refactor the entire codebase.

For each topic in ./topics spin up at least 1 subagent and more if there it is a big scope. Run the subagent and just asking it to load the topic and tell them to return with a report of what they found AND codesnippets of each of their proposals of before and after

When all reviewers are done create a HTML file in ai-docs/refactors
and if plannotator is available open the file with plannotator-annotate for the user to annotate. When done annotating process the feedback and treat all options that don't have a annotation telling you to not do it , revise , skip or any of the similar wording as approved and execute the refactors using a subagent with low thinking

Run these subagents asynchronously

Task / scope:
$ARGUMENTS

# Tests review

Refactor the requested tests so every surviving test proves behavior through a stable boundary and would catch a real production fault.

## Review output

Return a numbered list of suggestions; each item names the change and the production fault the surviving test still catches, followed by two fenced Markdown code blocks tagged with the source language:

This is what the code looks like before:

```LANGUAGE
the current test code, trimmed to the lines that change
```

and after:

```LANGUAGE
the proposed test code
```

Report test areas checked and left alone.

## Tests At Stable Boundaries

Tests should prove behavior through real boundaries. Do not test one-line helpers, duplicate implementation logic, or build large mock systems for small changes.

DON'T test the implementation sentence by sentence:

```python
assert server_id_for("Debian") == "wsl:Debian"
assert should_restart(available=False) is True
```

DO test the stable use-case boundary and observable order:

```python
await controller.update("Debian")
assert events == ["stop", "install", "verify", "start"]
```

## Prune And Compress

- Delete tests that prove nothing — assignment checks, mirror-the-implementation asserts, tests that cannot fail. Keep only tests that would catch a real regression.
- Make sure to see where we can use property based testing. Hypothesis in Python also to cleanup the current paramatrize or duplicated tests and to harden existing tests;
- Suggest the ecosystem's framework (fast-check, proptest, jqwik). Collapse hand-written case lists into properties and strengthen the surviving tests; note `hypothesis.stateful` exists, reach for it when a stateful model is genuinely warranted.
- Collapse further with pytest fixtures, shared fixtures in `conftest.py`, and `@pytest.mark.parametrize`.

## Completion Standard

Done when every suggestion names the production fault its surviving test would catch and the report ends with the estimated before/after per-module line delta.

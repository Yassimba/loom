## Python

### Code Quality (Python)

- Always add precise type hints.
  - Prefer dataclasses or Pydantic models over dictionaries for structured data.
  - Avoid bare types like `dict`, `list`, `tuple`, and `set` instead use `dict[str, int | str]`.
  - Prefer dataclasses or Pydantic models over dictionaries for structured data.
- Use top level imports
- Always use `uv add` to add dependencies never edit depedencies manually in `pyproject.toml`.

### Commands (Python)

After code changes, run the full quality gate and inspect complete output:

```bash
uv run ruff check
uv run ty check
uv run complexipy .
uv run tach check
```

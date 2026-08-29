#!/usr/bin/env python3
"""Deterministic context-load proxy for explain-code-flow."""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILL = ROOT / "skills/explain-code-flow"


def words(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").split())


skill_words = words(SKILL / "SKILL.md")
evidence_words = words(SKILL / "references/repository-evidence.md")
figure_selection_words = words(SKILL / "references/content-brief-by-type.md")
drawing_worker_words = sum(
    words(path)
    for path in (
        SKILL / "scripts/draw.py",
        SKILL / "scripts/example-figure.py",
        SKILL / "references/authoring-invariants.md",
    )
)
# Four figures is the documented usual case. Each independent worker receives
# the drawing packet, so repeated payload dominates prompt load.
prompt_words = skill_words + evidence_words + figure_selection_words + 4 * drawing_worker_words

for name, value in {
    "prompt_words": prompt_words,
    "skill_words": skill_words,
    "evidence_words": evidence_words,
    "drawing_worker_words": drawing_worker_words,
}.items():
    print(f"METRIC {name}={value}")

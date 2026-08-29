#!/usr/bin/env python3
"""Minimal figure script: connectors and labels first, then nodes, then write."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw import *  # noqa: E402,F401,F403

b = [
    hline(208, 120, 336, 120),
    label_above(272, 120, "Enter [valid]"),
    elbow([(408, 156), (408, 240), (656, 240)]),
    label_beside(416, 200, "Done"),
    state(64, 84, 144, 72, "Choose", "input"),
    state(336, 84, 144, 72, "Review", "validated", focal=True),
    ring(656, 240, "Installed"),
]

if __name__ == "__main__":
    stem = sys.argv[1] if len(sys.argv) > 1 else "example-flow"
    out = write(stem, "State machine", "Choose → Review → Installed",
                "State machine showing validated input reaching installation.",
                760, 320, "\n".join(b), project="example")
    print("wrote", out, "and", out.with_suffix(".svg"))

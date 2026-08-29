#!/usr/bin/env python3
"""Minimal one-source walkthrough bundle; compile with scripts/bundle.py."""
if "op" not in globals():
    from bundle import op

BUNDLE = {
    "version": 1,
    "title": "Example flow",
    "context": "A validated input reaches installation (`src/example.py:10`).",
    "figures": [
        {
            "stem": "01-overview",
            "eyebrow": "State machine",
            "title": "Choose → Review → Installed",
            "desc": "Validated input reaches installation.",
            "width": 760,
            "height": 320,
            "project": "example",
            "body": [
                op("hline", 208, 120, 336, 120),
                op("label_above", 272, 120, "Enter [valid]"),
                op("elbow", [(480, 120), (656, 120), (656, 240)]),
                op("label_beside", 664, 184, "Done"),
                op("state", 64, 84, 144, 72, "Choose", "input"),
                op("state", 336, 84, 144, 72, "Review", "validated", focal=True),
                op("ring", 656, 240, "Installed"),
            ],
        }
    ],
    "sections": [
        {
            "heading": "Overview",
            "claim": "Validated input reaches installation.",
            "figure": "01-overview",
            "alt": "Example state machine",
            "facts": ["Review accepts valid input (`src/example.py:10`)."],
        }
    ],
    "result": "The validated choice is installed.",
}

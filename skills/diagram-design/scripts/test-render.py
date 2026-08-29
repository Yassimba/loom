#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("diagram_render", SCRIPT_DIR / "render.py")
assert SPEC and SPEC.loader
render_module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(render_module)


class RenderTest(unittest.TestCase):
    def setUp(self) -> None:
        self.spec = {
            "version": 1,
            "type": "architecture",
            "slug": "request-flow",
            "title": "Request & flow",
            "description": "Browser request reaches storage.",
            "viewBox": [0, 0, 1000, 600],
            "zones": [{"label": "PRIVATE", "x": 320, "y": 64, "width": 560, "height": 400}],
            "nodes": [
                {"id": "web", "label": "Browser", "tag": "USER", "x": 80, "y": 160, "width": 160, "height": 80, "role": "input"},
                {"id": "api", "label": "API", "sublabel": "https:443", "tag": "API", "x": 400, "y": 160, "width": 160, "height": 80, "role": "focal"},
            ],
            "edges": [
                {"from": "web:right", "to": "api:left", "tone": "link", "label": {"text": "HTTPS", "x": 320, "y": 140}}
            ],
            "legend": [{"role": "focal", "label": "Primary service"}],
        }

    def test_renders_safe_accessible_html_that_passes_checks(self) -> None:
        html = render_module.render(self.spec)
        self.assertIn("Request &amp; flow", html)
        self.assertIn('aria-labelledby="request-flow-title request-flow-desc"', html)
        self.assertIn('marker-end="url(#arrow-link)"', html)
        self.assertIn("Primary service", html)
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "diagram.html"
            output.write_text(html, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT_DIR / "check.py"), str(output)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)

    def test_rejects_a_diagonal_route(self) -> None:
        self.spec["edges"][0]["via"] = [[300, 240]]
        with self.assertRaisesRegex(ValueError, "orthogonal"):
            render_module.render(self.spec)

    def test_supports_every_diagram_type_with_universal_primitives(self) -> None:
        references = SCRIPT_DIR.parent / "references"
        documented_types = {
            path.stem.removeprefix("type-")
            for path in references.glob("type-*.md")
            if path.name != "type-index.md"
        }
        self.assertEqual(documented_types, render_module.SUPPORTED_TYPES)
        self.assertEqual(39, len(render_module.SUPPORTED_TYPES))
        for diagram_type in render_module.SUPPORTED_TYPES:
            with self.subTest(diagram_type=diagram_type):
                spec = copy.deepcopy(self.spec)
                spec["type"] = diagram_type
                spec["nodes"] = []
                spec["edges"] = []
                spec["zones"] = []
                spec["legend"] = []
                spec["primitives"] = [
                    {"kind": "circle", "cx": 100, "cy": 100, "r": 24, "fill": "accent@10", "stroke": "accent"},
                    {"kind": "text", "x": 100, "y": 104, "text": diagram_type, "anchor": "middle"},
                ]
                html = render_module.render(spec)
                self.assertIn(f">{diagram_type}</text>", html)

    def test_cli_writes_output(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "diagram.json"
            output = Path(directory) / "diagram.html"
            source.write_text(json.dumps(self.spec), encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT_DIR / "render.py"), str(source), "--output", str(output)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            self.assertTrue(output.is_file())


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3

from __future__ import annotations

import copy
from importlib import import_module, util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
diagram_profile = import_module("diagram_profile")
SPEC = util.spec_from_file_location("diagram_render", SCRIPT_DIR / "render.py")
assert SPEC and SPEC.loader
render_module = util.module_from_spec(SPEC)
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

    def test_rejects_a_diagonal_route_with_a_targeted_fix(self) -> None:
        self.spec["edges"][0]["via"] = [[300, 240]]
        with self.assertRaisesRegex(
            ValueError, r"edges\[0\].*not orthogonal; add via"
        ):
            render_module.render(self.spec)

    def test_reports_every_invalid_edge_in_one_pass(self) -> None:
        self.spec["edges"][0]["via"] = [[300, 240]]
        self.spec["edges"].append(
            {"from": "api:left", "to": "web:right", "via": [[320, 260]]}
        )
        with self.assertRaises(ValueError) as raised:
            render_module.render(self.spec)
        self.assertIn("edges[0]", str(raised.exception))
        self.assertIn("edges[1]", str(raised.exception))

    def test_ignores_repeated_connector_points(self) -> None:
        self.spec["edges"][0]["via"] = [[400, 200], [400, 200]]
        html = render_module.render(self.spec)
        self.assertIn('d="M 240 200 H 400"', html)

    def test_rejects_a_node_label_that_will_clip(self) -> None:
        self.spec["nodes"][0]["width"] = 80
        self.spec["nodes"][0]["label"] = "A very long process label"
        with self.assertRaisesRegex(ValueError, "label is likely clipped"):
            render_module.render(self.spec)

    def test_renders_and_validates_diamond_nodes(self) -> None:
        self.spec["nodes"][1]["shape"] = "diamond"
        html = render_module.render(self.spec)
        self.assertIn('points="480,160 560,200 480,240 400,200"', html)

        self.spec["nodes"][1]["shape"] = "hexagon"
        with self.assertRaisesRegex(
            ValueError, "node api shape must be 'rectangle' or 'diamond'"
        ):
            render_module.render(self.spec)

    def test_invalid_paint_reports_its_path_and_allowed_values(self) -> None:
        self.spec["primitives"] = [
            {"kind": "line", "x1": 0, "y1": 0, "x2": 40, "y2": 0, "stroke": "line"}
        ]
        with self.assertRaisesRegex(
            ValueError,
            r"primitives\[0\]\.stroke: invalid paint 'line'; expected a profile token",
        ):
            render_module.render(self.spec)

    def test_supports_secondary_profile_paints(self) -> None:
        self.spec["primitives"] = [
            {
                "kind": "rect",
                "x": 0,
                "y": 0,
                "width": 40,
                "height": 20,
                "fill": "paper-2",
                "stroke": "rule-solid",
            }
        ]
        html = render_module.render(self.spec)
        self.assertIn('fill="#ececec" stroke="#bfc0c0"', html)

    def test_emits_review_metadata_on_drawable_elements(self) -> None:
        self.spec["nodes"][0]["code"] = ["src/web.ts:4-8", "src/web.ts:12-14"]
        self.spec["nodes"][0]["change"] = "modified"
        self.spec["edges"][0]["code"] = "src/api.ts:20-24"
        self.spec["edges"][0]["change"] = "added"
        self.spec["zones"][0]["change"] = "same"
        self.spec["primitives"] = [
            {
                "kind": "circle",
                "cx": 40,
                "cy": 40,
                "r": 12,
                "change": "projected",
            }
        ]
        html = render_module.render(self.spec)
        self.assertIn(
            'data-code="src/web.ts:4-8,src/web.ts:12-14" data-change="modified"',
            html,
        )
        self.assertIn('data-code="src/api.ts:20-24" data-change="added"', html)
        self.assertIn('data-change="same"', html)
        self.assertIn('data-change="projected"', html)

    def test_rejects_unsafe_or_bound_projected_metadata(self) -> None:
        self.spec["nodes"][0]["code"] = "../secret.ts:1-2"
        with self.assertRaisesRegex(ValueError, "invalid code binding"):
            render_module.render(self.spec)

        self.spec["nodes"][0]["code"] = "src/web.ts:1-2"
        self.spec["nodes"][0]["change"] = "projected"
        with self.assertRaisesRegex(ValueError, "projected elements must not have code"):
            render_module.render(self.spec)

    def test_supports_every_diagram_type_with_universal_primitives(self) -> None:
        references = SCRIPT_DIR.parent / "references"
        documented_types = {
            path.stem.removeprefix("type-")
            for path in references.glob("type-*.md")
            if path.name != "type-index.md"
        }
        self.assertEqual(documented_types, render_module.SUPPORTED_TYPES)
        self.assertEqual(41, len(render_module.SUPPORTED_TYPES))
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

    def test_profile_resolution_applies_project_tokens_and_fonts(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            home = root / "home"
            project = root / "project"
            profile_dir = home / ".diagram-design/profiles"
            project.mkdir()
            profile_dir.mkdir(parents=True)
            project.joinpath(".diagram-design").write_text("profile: acme\n", encoding="utf-8")
            guide = diagram_profile.STYLE_GUIDE.read_text(encoding="utf-8")
            guide = guide.replace("#eb6c36", "#123456").replace("| `node-name` | Geist (sans) |", "| `node-name` | Inter (sans) |")
            profile_dir.joinpath("acme.md").write_text(guide, encoding="utf-8")

            profile = diagram_profile.resolve_profile(project, home)
            self.assertEqual("#123456", profile.tokens["accent"])
            self.assertEqual("Inter", profile.fonts["sans"])
            html = render_module.render(self.spec, profile)
            self.assertIn("#123456", html)
            self.assertIn("'Inter', system-ui", html)

    def test_malformed_marker_falls_back_with_a_warning(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            project.joinpath(".diagram-design").write_text("profile: ../unsafe\n", encoding="utf-8")
            profile = diagram_profile.resolve_profile(project)
            self.assertEqual(diagram_profile.STYLE_GUIDE, profile.source)
            self.assertTrue(profile.warnings)

    def test_missing_default_snapshot_uses_shipped_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / "project"
            home = root / "home"
            project.mkdir()
            project.joinpath(".diagram-design").write_text("profile: default\n", encoding="utf-8")
            profile = diagram_profile.resolve_profile(project, home)
            self.assertEqual(diagram_profile.STYLE_GUIDE, profile.source)
            self.assertTrue(profile.warnings)

    def test_build_command_renders_and_checks_once(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "diagram.json"
            output = root / "diagram.html"
            source.write_text(json.dumps(self.spec), encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT_DIR / "build.py"),
                    str(source),
                    "--output",
                    str(output),
                    "--project-root",
                    str(root),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            self.assertIn("build passed", result.stdout)
            self.assertTrue(output.is_file())


if __name__ == "__main__":
    unittest.main()

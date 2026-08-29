#!/usr/bin/env python3
"""Small regression suite for the explanation bundle compiler."""
from __future__ import annotations

import os
import runpy
import subprocess
import tempfile
import unittest
from pathlib import Path

import bundle

HERE = Path(__file__).resolve().parent


class BundleTests(unittest.TestCase):
    def test_example_is_valid_and_renderable(self) -> None:
        figure = runpy.run_path(
            str(HERE / "example-bundle.py"), init_globals={"op": bundle.op}
        )["BUNDLE"]["figures"][0]
        self.assertEqual(bundle._geometry(figure), ([], []))
        errors: list[str] = []
        rendered = bundle._resolve(figure["body"], errors, "example")
        self.assertFalse(errors)
        self.assertTrue(all(isinstance(item, str) for item in rendered))

    def test_primitive_errors_are_aggregated(self) -> None:
        errors: list[str] = []
        bundle._resolve(
            [bundle.op("missing", 1), bundle.op("node", 0, 0)], errors, "figure"
        )
        self.assertEqual(len(errors), 2)

    def test_geometry_reports_overlap_route_and_newline(self) -> None:
        figure = {
            "width": 320,
            "height": 200,
            "body": [
                bundle.op("node", 40, 40, 120, 64, "first\nline"),
                bundle.op("node", 80, 60, 120, 64, "second"),
                bundle.op("hline", 0, 72, 300, 72),
            ],
        }
        errors, _warnings = bundle._geometry(figure)
        text = "\n".join(errors)
        self.assertIn("boxes overlap", text)
        self.assertIn("connector crosses", text)
        self.assertIn("literal newline", text)

    def test_shared_checker_propagates_geometry_failure(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            diagrams = root / "diagrams"
            diagrams.mkdir()
            html = diagrams / "bad.html"
            html.write_text(
                '<html><body><svg role="img" aria-labelledby="bad-title bad-desc">'
                '<title id="bad-title">Bad</title><desc id="bad-desc">Bad geometry</desc>'
                '<rect x="90" y="96" width="40" height="12" fill="white"/>'
                '<rect x="100" y="100" width="120" height="60" fill="white"/>'
                '</svg></body></html>', encoding="utf-8"
            )
            (diagrams / "bad.svg").write_text(
                '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>',
                encoding="utf-8",
            )
            fake_bin = root / "bin"
            fake_bin.mkdir()
            converter = fake_bin / "rsvg-convert"
            converter.write_text(
                '#!/bin/sh\nout=""\nwhile [ "$#" -gt 0 ]; do '
                '[ "$1" = "-o" ] && { shift; out="$1"; }; shift; done\n'
                'touch "$out"\n', encoding="utf-8"
            )
            converter.chmod(0o755)
            env = os.environ | {"PATH": f"{fake_bin}:{os.environ.get('PATH', '')}"}
            result = subprocess.run(
                [str(HERE / "check-figures.sh"), str(diagrams)],
                env=env, capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("label mask", result.stdout)


if __name__ == "__main__":
    unittest.main()

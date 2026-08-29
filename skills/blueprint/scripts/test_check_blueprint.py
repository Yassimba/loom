from __future__ import annotations

import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import ModuleType

SCRIPT = Path(__file__).with_name("check-blueprint.py")


def load_validator() -> ModuleType:
    spec = importlib.util.spec_from_file_location("check_blueprint", SCRIPT)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load Blueprint validator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


validator = load_validator()


class BlueprintValidatorContractTest(unittest.TestCase):
    def test_runtime_svg_path_grammar_is_shared_by_the_validator(self) -> None:
        self.assertTrue(validator.valid_svg_path("ai-docs/blueprints/x/diagrams/a.svg"))
        for path in ("", "a/unsafe\0.svg", "../a.svg", "a/../b.svg", "/a.svg", "C:/a.svg", "a\\b.svg", "a.svg?x=1", "a.svg#x", "a.txt"):
            with self.subTest(path=path):
                self.assertFalse(validator.valid_svg_path(path))

    def test_directive_parser_rejects_nonempty_and_unclosed_fences(self) -> None:
        valid = '```plannotator-svg path="diagrams/a.svg"\n```'
        self.assertEqual(validator.plan_directives(valid), (["diagrams/a.svg"], []))
        for invalid in (
            '```plannotator-svg path="diagrams/a.svg"',
            '```plannotator-svg path="diagrams/a.svg"\nbody\n```',
            '```plannotator-svg path="diagrams/a.svg"\n``` trailing',
        ):
            with self.subTest(invalid=invalid):
                self.assertTrue(validator.plan_directives(invalid)[1])

    def test_retained_artifact_policy_keeps_only_contract_files_and_final_svgs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            blueprint = Path(temporary)
            (blueprint / "plan.md").write_text("# Plan\n", encoding="utf-8")
            (blueprint / "diagrams").mkdir()
            (blueprint / "diagrams/final.svg").write_text("<svg/>", encoding="utf-8")
            self.assertEqual(validator.retained_artifacts(blueprint)[1], [])
            (blueprint / "contact-sheet.png").write_bytes(b"png")
            self.assertEqual(validator.retained_artifacts(blueprint)[1], ["contact-sheet.png"])

    def test_lock_hashes_the_repository_before_creating_lock_outputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            blueprint = root / "ai-docs/blueprints/example"
            blueprint.mkdir(parents=True)
            (blueprint / "plan.md").write_text("# Plan\n", encoding="utf-8")
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "add", "ai-docs/blueprints/example/plan.md"], check=True)
            subprocess.run([
                "git", "-C", str(root), "-c", "commit.gpgsign=false", "-c", "user.name=Test",
                "-c", "user.email=test@example.com", "commit", "-qm", "test fixture",
            ], check=True)
            expected = validator.baseline_sha256(root)
            validator.lock(blueprint, root)
            record = json.loads((blueprint / "approval.json").read_text(encoding="utf-8"))
            self.assertEqual(record["baselineSha256"], expected)


if __name__ == "__main__":
    unittest.main()

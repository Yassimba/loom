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
    def test_compact_mermaid_plan_locks_and_detects_changed_figure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "source.py").write_text("pass\n")
            subprocess.run(["git", "-C", str(root), "add", "source.py"], check=True)
            subprocess.run(["git", "-C", str(root), "-c", "commit.gpgsign=false", "-c", "user.name=Test",
                            "-c", "user.email=test@example.com", "commit", "-qm", "fixture"], check=True)
            head = validator.run_git(root, "rev-parse", "HEAD").decode().strip()
            blueprint = root / "ai-docs/blueprints/example"
            (blueprint / "diagrams").mkdir(parents=True)
            svg = blueprint / "diagrams/flow.svg"
            svg.write_text('<svg viewBox="0 0 10 10"><text>PROJECTED</text></svg>')
            (blueprint / "diagrams/flow.mmd").write_text('flowchart LR\n A --> B["PROJECTED"]\n')
            plan = "\n".join(f"## {heading}\nContent.\n" for heading in
                             ("Intent", "Acceptance criteria", "Changes", "Implementation", "Verification", "Risks"))
            plan += '\nC1: add B; verify B.\n```plannotator-svg path="ai-docs/blueprints/example/diagrams/flow.svg"\n```\n'
            (blueprint / "plan.md").write_text(plan)
            (blueprint / "overlay.json").write_text(json.dumps({"version": 2, "target": {
                "head": head, "baselineSha256": validator.baseline_sha256(root, blueprint)},
                "atlas": None, "figures": [{"id": "flow", "output": "diagrams/flow.svg", "mermaid": "diagrams/flow.mmd"}]}))
            self.assertEqual(validator.validate(blueprint, root), [])
            (root / "source.py").write_text("changed\n")
            self.assertTrue(any("baseline changed" in error for error in validator.validate(blueprint, root)))
            (root / "source.py").write_text("pass\n")
            validator.lock(blueprint, root)
            self.assertEqual(validator.validate(blueprint, root), [])
            svg.write_text('<svg viewBox="0 0 10 10"><text>Different promise</text></svg>')
            self.assertTrue(any("approved artifacts changed" in error for error in validator.validate(blueprint, root)))

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

    def test_source_ranges_are_exact_and_within_the_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "source.py").write_text("one\ntwo\n", encoding="utf-8")
            errors: list[str] = []
            validator.validate_source_range("source.py:1-2", root, errors, "test")
            self.assertEqual(errors, [])
            for value in ("source.py:2-1", "source.py:1-3", "source.py:1", "missing.py:1-1"):
                with self.subTest(value=value):
                    errors = []
                    validator.validate_source_range(value, root, errors, "test")
                    self.assertTrue(errors)

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

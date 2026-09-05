"""Integration checks for atlas retrieval, incremental publication, and failures."""
import copy
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

import atlas


class AtlasTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / "repo"
        self.repo.mkdir()
        atlas.git(self.repo, "init", "-q")
        (self.repo / "source.py").write_text("def save():\n    return 1\n")
        self.commit()
        self.root = self.repo / "ai-docs/atlas"
        (self.root / "topics").mkdir(parents=True)
        (self.root / "diagrams/api").mkdir(parents=True)
        self.pin = atlas.git(self.repo, "rev-parse", "HEAD").strip()
        atlas.write(self.root / "atlas.json", {"title": "Example", "repositories": [
            {"id": "api", "path": "../..", "commit": self.pin}]})
        self.topic = {
            "id": "api.save", "section": "api", "title": "Saving", "summary": "Persists a request.",
            "questions": ["Where do requests become durable?"], "terms": ["durability"],
            "facts": [{"id": "save", "text": "save returns one", "sources": ["save"]}],
            "sources": [{"id": "save", "repo": "api", "path": "source.py", "symbol": "save",
                         "start": 1, "end": 2, "anchor": "def save():"}],
            "figures": [{"id": "save-flow", "json": "diagrams/api/save.json", "question": "Where is save?"}],
        }
        atlas.write(self.root / "topics/save.json", self.topic)
        atlas.write(self.root / "diagrams/api/save.json", {
            "nodes": [{"id": "save", "label": "save()", "code": ["source.py:1-2"]}]})
        (self.root / "diagrams/api/save.html").write_text(
            '<svg viewBox="0 0 100 100"><g data-element-id="save" data-code="source.py:1-2"><text>save</text></g></svg>')
        atlas.write(self.root / "diagrams/api/manifest.json", {
            "section": "api", "title": "API", "order": 1,
            "diagrams": [{"id": "save-flow", "repo": "api", "file": "save.html", "json": "save.json",
                          "title": "Save", "type": "flowchart", "level": 1}],
            "typeDecisions": [{"type": name, "reason": "Tiny fixture"} for name in atlas.catalogue_types()],
            "coverage": ["save"], "depthCheck": "No hidden behavior", "quotaReason": "One-function fixture"})

    def commit(self):
        atlas.git(self.repo, "add", "source.py")
        atlas.git(self.repo, "-c", "user.name=Test", "-c", "user.email=test@example.com",
                  "-c", "commit.gpgsign=false", "commit", "-qm", "fixture")

    def test_search_show_and_no_change_refresh(self):
        self.assertEqual(len(atlas.catalogue_types()), 39)
        self.assertIn("Architecture", atlas.catalogue_types())
        atlas.validate(self.root)
        self.assertEqual(atlas.search(self.root, "durability", 1)[0]["id"], "api.save")
        shown = atlas.show(self.root, "api.save", 0, 1)
        self.assertEqual(shown["facts"], self.topic["facts"])
        self.assertNotIn("elements", shown["figures"][0])
        self.assertNotIn("x", atlas.show_figure(self.root, "save-flow", 0, 1)["elements"][0])
        before = atlas.tree_hash(self.root)
        self.assertEqual(atlas.prepare(self.root, {}), {"status": "unchanged"})
        self.assertEqual(atlas.tree_hash(self.root), before)

    def test_refresh_repairs_anchors_and_keeps_failed_publication_untouched(self):
        (self.repo / "source.py").write_text("# moved\ndef save():\n    return 2\n")
        self.commit()
        before = atlas.tree_hash(self.root)
        preparation = atlas.prepare(self.root, {})
        self.assertEqual(preparation["topics"], ["api.save"])
        stage = Path(preparation["stage"])
        with self.assertRaisesRegex(ValueError, "review all"):
            atlas.publish(stage)
        record = atlas.read(stage / "refresh.json")
        record.update(reviewed=True, decisions=[{"repo": "api", "path": "source.py", "reason": "Updated save"}])
        atlas.write(stage / "refresh.json", record)
        topic = atlas.read(stage / "topics/save.json")
        topic["sources"][0]["anchor"] = "not in source"
        atlas.write(stage / "topics/save.json", topic)
        with self.assertRaisesRegex(ValueError, "stale source"):
            atlas.publish(stage)
        self.assertEqual(atlas.tree_hash(self.root), before)
        topic["sources"][0].update(start=2, end=3, anchor="def save():")
        topic["facts"][0]["text"] = "save returns two"
        atlas.write(stage / "topics/save.json", topic)
        with self.assertRaisesRegex(ValueError, "unverified figure binding"):
            atlas.publish(stage)
        diagram = atlas.read(stage / "diagrams/api/save.json")
        diagram["nodes"][0]["code"] = ["source.py:2-3"]
        atlas.write(stage / "diagrams/api/save.json", diagram)
        with self.assertRaisesRegex(ValueError, "rendered bindings differ"):
            atlas.publish(stage)
        (stage / "diagrams/api/save.html").write_text(
            '<svg viewBox="0 0 100 100"><g data-element-id="save" data-code="source.py:2-3"><text>save</text></g></svg>')
        output = atlas.publish(stage)
        self.assertEqual(atlas.tree_hash(Path(output["previous"])), before)
        self.assertNotEqual(atlas.repositories(self.root)["api"]["commit"], self.pin)
        page = (self.root / "atlas.html").read_text()
        self.assertIn("durability", page)
        self.assertIn('id="topic-api.save"', page)
        self.assertEqual(atlas.search(self.root, "two", 1)[0]["id"], "api.save")

    def test_fact_page_does_not_emit_unrelated_sources_or_geometry(self):
        for index in range(100):
            self.topic["sources"].append({"id": f"other{index}", "repo": "api", "path": "source.py",
                                          "start": 1, "end": 2, "anchor": "def save():"})
            self.topic["facts"].append({"id": f"f{index}", "text": "Other detail", "sources": [f"other{index}"]})
        atlas.write(self.root / "topics/save.json", self.topic)
        result = atlas.show(self.root, "api.save", 0, 1)
        self.assertEqual(len(result["sources"]), 1)
        self.assertEqual(result["nextOffset"], 1)
        self.assertLess(len(json.dumps(result)), 1000)

    def test_binding_on_the_wrong_element_is_rejected(self):
        path = self.root / "diagrams/api/save.html"
        path.write_text(path.read_text().replace('data-element-id="save"', 'data-element-id="other"'))
        with self.assertRaisesRegex(ValueError, "rendered bindings differ"):
            atlas.validate(self.root)

    def test_unmapped_new_files_and_cross_repository_dependencies(self):
        other_repo = Path(self.temp.name) / "consumer"
        subprocess.run(["git", "clone", "-q", str(self.repo), str(other_repo)], check=True)
        config = atlas.read(self.root / "atlas.json")
        config["repositories"].append({"id": "consumer", "path": str(other_repo), "commit": self.pin})
        atlas.write(self.root / "atlas.json", config)
        other = copy.deepcopy(self.topic)
        other.update(id="consumer.save", dependsOn=["api.save"])
        other["sources"][0]["repo"] = "consumer"
        atlas.write(self.root / "topics/consumer.json", other)
        (self.repo / "source.py").write_text("def save():\n    return 2\n")
        (self.repo / "new.py").write_text("def new(): pass\n")
        atlas.git(self.repo, "add", "new.py")
        self.commit()
        result = atlas.affected(self.root, {})
        self.assertEqual(result["topics"], ["api.save", "consumer.save"])
        self.assertEqual([row["path"] for row in result["unmapped"]], ["new.py"])
        self.assertEqual(result["targets"]["consumer"], self.pin)

    def test_rename_and_delete_are_reported(self):
        atlas.git(self.repo, "mv", "source.py", "renamed.py")
        atlas.git(self.repo, "-c", "user.name=Test", "-c", "user.email=test@example.com",
                  "-c", "commit.gpgsign=false", "commit", "-qm", "rename")
        changed = atlas.affected(self.root, {})["changes"][0]
        self.assertEqual(changed["oldPath"], "source.py")
        self.assertEqual(changed["topics"], ["api.save"])
        (self.repo / "renamed.py").unlink()
        atlas.git(self.repo, "add", "-u")
        atlas.git(self.repo, "-c", "user.name=Test", "-c", "user.email=test@example.com",
                  "-c", "commit.gpgsign=false", "commit", "-qm", "delete")
        self.assertEqual(atlas.affected(self.root, {})["changes"][0]["status"], "D")

    def test_publish_rejects_concurrent_atlas_edit(self):
        (self.repo / "source.py").write_text("def save():\n    return 2\n")
        self.commit()
        stage = Path(atlas.prepare(self.root, {})["stage"])
        record = atlas.read(stage / "refresh.json")
        record.update(reviewed=True, decisions=[{"repo": "api", "path": "source.py", "reason": "Reviewed"}])
        atlas.write(stage / "refresh.json", record)
        (self.root / "human-note.txt").write_text("Keep this")
        with self.assertRaisesRegex(ValueError, "changed since prepare"):
            atlas.publish(stage)


if __name__ == "__main__":
    unittest.main()

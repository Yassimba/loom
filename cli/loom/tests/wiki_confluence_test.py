"""Run with the pinned CME virtualenv Python; uses only isolated temporary config."""
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

ADAPTER = Path(__file__).resolve().parents[1] / "src/wiki_confluence.py"


class ConfluenceConfigTest(unittest.TestCase):
    def test_private_atomic_merge_rejects_unsafe_and_changed_config(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            path = root / "config.json"
            env = {**os.environ, "CME_CONFIG_PATH": str(path), "HOME": str(root)}
            request = {"url": "https://company.atlassian.net/wiki/", "username": "email@example.test",
                       "token": "SECRET-FIXTURE", "pat": False, "action": "inspect"}

            def run(**updates):
                process = subprocess.run([sys.executable, str(ADAPTER)], input=json.dumps({**request, **updates}),
                                         text=True, capture_output=True, env=env, timeout=10)
                self.assertNotIn("SECRET-FIXTURE", process.stdout + process.stderr)
                self.assertNotIn("SECRET-OTHER", process.stdout + process.stderr)
                return process, json.loads(process.stdout)

            other = {"auth": {"confluence": {"https://other.test": {"pat": "SECRET-OTHER"}},
                              "jira": {"https://jira.test": {"pat": "keep"}}}, "unrelated": {"keep": True}}
            path.write_text(json.dumps(other))
            before = path.read_bytes()
            process, inspected = run()
            self.assertEqual(process.returncode, 0)
            self.assertFalse(inspected["exists"])
            self.assertEqual(path.read_bytes(), before)
            process, saved = run(action="save", digest=inspected["digest"])
            self.assertEqual(process.returncode, 0)
            self.assertTrue(saved["saved"])
            data = json.loads(path.read_text())
            self.assertEqual(data["unrelated"], other["unrelated"])
            self.assertEqual(data["auth"]["jira"], other["auth"]["jira"])
            self.assertEqual(data["auth"]["confluence"]["https://other.test"], {"pat": "SECRET-OTHER"})
            account = data["auth"]["confluence"][request["url"].rstrip("/")]
            self.assertEqual(account["api_token"], "SECRET-FIXTURE")
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            before = path.read_bytes()
            _, inspected = run()
            self.assertTrue(inspected["exists"])
            self.assertNotEqual(run(action="save", digest=inspected["digest"])[0].returncode, 0)
            self.assertEqual(path.read_bytes(), before)
            process, _ = run(action="save", digest=inspected["digest"], replace=True, pat=True, username="")
            self.assertEqual(process.returncode, 0)
            account = json.loads(path.read_text())["auth"]["confluence"][request["url"].rstrip("/")]
            self.assertEqual(account["pat"], "SECRET-FIXTURE")
            self.assertEqual(account["api_token"], "")
            self.assertNotEqual(run(action="save", digest=inspected["digest"], replace=True)[0].returncode, 0)
            for bad in ('{"token": "SECRET-FIXTURE",', '{"auth": {"confluence": 42}}', '[]'):
                path.write_text(bad)
                self.assertNotEqual(run()[0].returncode, 0)
                self.assertEqual(path.read_text(), bad)
            path.unlink()
            target = root / "target.json"
            target.write_text("{}")
            path.symlink_to(target)
            self.assertNotEqual(run()[0].returncode, 0)
            self.assertEqual(target.read_text(), "{}")
            self.assertEqual(list(root.glob(".loom-cme-*")), [])


if __name__ == "__main__":
    unittest.main()

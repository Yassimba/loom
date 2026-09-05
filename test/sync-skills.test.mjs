import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cp, lstat, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

test("sync-skills link removes only stale repo links", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "loom-sync-stale-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const repo = join(root, "repo");
  const home = join(root, "home");
  const tree = join(home, ".claude/skills");
  await mkdir(join(repo, "scripts"), { recursive: true });
  await mkdir(join(repo, "skills/current"), { recursive: true });
  await mkdir(join(tree, "custom"), { recursive: true });
  await cp(resolve("scripts/sync-skills.sh"), join(repo, "scripts/sync-skills.sh"));
  await writeFile(join(repo, "skills/current/SKILL.md"), "# current\n");
  await writeFile(join(tree, "custom/SKILL.md"), "# custom\n");
  await symlink(join(repo, "skills/removed"), join(tree, "removed"));
  await symlink(join(root, "foreign"), join(tree, "foreign"));

  const result = spawnSync(
    "bash",
    [join(repo, "scripts/sync-skills.sh"), "link", "--tree", "claude"],
    { env: { ...process.env, HOME: home }, encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /remove\s+removed\s+\(stale repo link\)/);
  await assert.rejects(lstat(join(tree, "removed")), { code: "ENOENT" });
  assert.equal((await lstat(join(tree, "foreign"))).isSymbolicLink(), true);
  assert.equal(await readFile(join(tree, "custom/SKILL.md"), "utf8"), "# custom\n");
  assert.equal((await lstat(join(tree, "current"))).isSymbolicLink(), true);
});

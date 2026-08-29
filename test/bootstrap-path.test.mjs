import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

test("the Unix bootstrap persists mise activation exactly once", async () => {
  const root = await mkdtemp(join(tmpdir(), "loom-bootstrap-path-"));
  const home = join(root, "home");
  const bin = join(root, "bin");
  await mkdir(home);
  await mkdir(bin);
  await writeFile(
    join(bin, "mise"),
    `#!/bin/sh
if [ "$1" = "-C" ]; then shift 2; fi
case "$1" in
  doctor) echo "activated: no" ;;
  install) exit 0 ;;
  exec) printf '%s\\n' "$@" > "$HOME/mise-exec-args" ;;
esac
`,
  );
  await writeFile(
    join(bin, "curl"),
    `#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then output="$2"; shift 2; else shift; fi
done
printf '%s\\n' '[tools]' '# core:begin' '"github:Yassimba/loom[exe=loom]" = "loom-v0.9.0"' '# core:end' > "$output"
`,
  );
  await Promise.all([chmod(join(bin, "mise"), 0o755), chmod(join(bin, "curl"), 0o755)]);

  const selection = join(home, ".config", "mise", "conf.d", "loom.toml");
  await mkdir(dirname(selection), { recursive: true });
  await writeFile(selection, '[tools]\ntokei = "12.1.2"\n');

  try {
    const env = {
      ...process.env,
      HOME: home,
      SHELL: "/bin/bash",
      PATH: `${bin}:/usr/bin:/bin`,
    };
    for (let run = 0; run < 2; run += 1) {
      const result = spawnSync(
        "sh",
        [join(repoRoot, "install.sh"), "--skill", "tdd", "--agent", "codex", "--yes"],
        {
          encoding: "utf8",
          env,
        },
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      // The new-shell hint appears exactly when activation was just added.
      assert.equal(result.stdout.includes("open a new shell"), run === 0, result.stdout);
    }

    const bashrc = await readFile(join(home, ".bashrc"), "utf8");
    assert.equal(
      bashrc.match(/eval "\$\("[^"]+\/mise" activate bash\)"/g)?.length,
      1,
      "expected one persistent mise activation",
    );
    assert.match(bashrc, /export PATH="[^"]+:\$PATH"/);
    assert.match(bashrc, new RegExp(join(bin, "mise").replaceAll("/", "\\/")));
    assert.deepEqual((await readFile(join(home, "mise-exec-args"), "utf8")).trim().split("\n"), [
      "exec",
      "--",
      "loom",
      "setup",
      "--skill",
      "tdd",
      "--agent",
      "codex",
      "--yes",
    ]);
    const repairedSelection = await readFile(selection, "utf8");
    assert.match(repairedSelection, /# core:begin/);
    assert.match(repairedSelection, /github:Yassimba\/loom\[exe=loom\]/);
    assert.match(repairedSelection, /tokei = "12\.1\.2"/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("the PowerShell bootstrap persists mise activation idempotently", async () => {
  const script = await readFile(join(repoRoot, "install.ps1"), "utf8");

  assert.match(script, /\$MiseExe = \(Get-Command mise/);
  assert.match(script, /\$Activation = ".*\$MiseExe' activate pwsh\)/);
  assert.match(script, /\.Contains\(\$Activation\)/);
  assert.match(script, /Add-Content -Path \$PROFILE -Value \$Activation/);
});

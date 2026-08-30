import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmod, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

async function unixFixture(label, mise, manifestLine, extras = {}) {
  const root = await mkdtemp(join(tmpdir(), `loom-bootstrap-${label}-`));
  const home = join(root, "home");
  const bin = join(root, "bin");
  await Promise.all([mkdir(home), mkdir(bin)]);
  const curl = `#!/bin/sh
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then output="$2"; shift 2; else shift; fi
done
printf '%s\\n' '[tools]' '# core:begin' '${manifestLine}' '# core:end' > "$output"
`;
  const executables = { mise, curl, ...extras };
  await Promise.all(
    Object.entries(executables).map(async ([name, content]) => {
      const path = join(bin, name);
      await writeFile(path, content);
      await chmod(path, 0o755);
    }),
  );
  return {
    root,
    home,
    bin,
    env: { ...process.env, HOME: home, SHELL: "/bin/bash", PATH: `${bin}:/usr/bin:/bin` },
  };
}

test("the Unix bootstrap persists mise activation exactly once", async () => {
  const { root, home, bin, env } = await unixFixture(
    "path",
    `#!/bin/sh
if [ "$1" = "-C" ]; then shift 2; fi
case "$1" in
  data) [ "$2" = "dir" ] && printf '%s\\n' "$HOME/.local/share/mise" || exit 64 ;;
  install) exit 0 ;;
  exec) printf '%s\\n' "$@" > "$HOME/mise-exec-args" ;;
  *) exit 64 ;;
esac
`,
    '"github:Yassimba/loom[exe=loom]" = "loom-v0.9.0"',
  );

  const selection = join(home, ".config", "mise", "conf.d", "loom.toml");
  await mkdir(dirname(selection), { recursive: true });
  await writeFile(selection, '[tools]\ntokei = "12.1.2"\n');

  try {
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

test("the Unix bootstrap stops when mise cannot report its data directory", async () => {
  const { root, home, env } = await unixFixture(
    "mise-data",
    `#!/bin/sh
if [ "$1" = "data" ] && [ "$2" = "dir" ]; then exit 73; fi
if [ "$1" = "-C" ]; then exit 0; fi
exit 64
`,
    'node = "24.19.0"',
  );

  try {
    const result = spawnSync(
      "sh",
      [join(repoRoot, "install.sh"), "--skill", "tdd", "--agent", "agents", "--yes"],
      {
        encoding: "utf8",
        env: { ...env, LOOM_E2E_LOOM_BIN: join(home, "e2e-loom-must-not-run") },
      },
    );

    assert.equal(result.status, 73, result.stderr || result.stdout);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("the Unix bootstrap keeps its new-shell guidance when setup fails", async () => {
  const { root, bin, env } = await unixFixture(
    "setup-failure",
    `#!/bin/sh
if [ "$1" = "data" ] && [ "$2" = "dir" ]; then echo "$HOME/.local/share/mise"; exit 0; fi
if [ "$1" = "-C" ]; then exit 0; fi
exit 64
`,
    'node = "24.19.0"',
    { "loom-fail": "#!/bin/sh\nexit 9\n" },
  );

  try {
    const result = spawnSync(
      "sh",
      [join(repoRoot, "install.sh"), "--skill", "tdd", "--agent", "agents", "--yes"],
      {
        encoding: "utf8",
        env: { ...env, LOOM_E2E_LOOM_BIN: join(bin, "loom-fail") },
      },
    );

    assert.equal(result.status, 9, result.stderr || result.stdout);
    assert.match(result.stdout, /open a new shell/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("the PowerShell bootstrap persists mise activation idempotently", async () => {
  const script = await readFile(join(repoRoot, "install.ps1"), "utf8");

  assert.match(script, /\$MiseCommand = \(Get-Command mise/);
  assert.match(script, /\$Activation = ".*\$MiseExe' activate pwsh\)/);
  assert.match(script, /\.Contains\(\$Activation\)/);
  assert.match(script, /Set-AtomicLines \$ProfilePath \$ProfileLines/);
  assert.match(script, /GetFolderPath\(\[Environment\+SpecialFolder\]::MyDocuments\)/);
  assert.match(script, /Join-Path \$Documents "WindowsPowerShell\\profile\.ps1"/);
  assert.match(script, /Join-Path \$Documents "PowerShell\\profile\.ps1"/);
  assert.match(script, /activate pwsh\) \| Out-String \| Invoke-Expression/);
  assert.match(script, /Select-Object -Unique/);
  assert.match(script, /open a new PowerShell, or run: \. `\$PROFILE/);
});

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

test("PowerShell refreshes persisted PATH before resolving mise", async () => {
  const installer = await readFile(join(repoRoot, "install.ps1"), "utf8");
  const wingetInstall = installer.indexOf("winget install");
  const machinePath = installer.indexOf(
    "[System.EnvironmentVariableTarget]::Machine",
    wingetInstall + 1,
  );
  const userPath = installer.indexOf("[System.EnvironmentVariableTarget]::User", wingetInstall + 1);
  const miseGuard = installer.indexOf(
    "if (-not (Get-Command mise -ErrorAction SilentlyContinue))",
    wingetInstall + 1,
  );

  assert.ok(wingetInstall >= 0, "expected the WinGet install path");
  assert.ok(machinePath > wingetInstall && machinePath < miseGuard);
  assert.ok(userPath > wingetInstall && userPath < miseGuard);
});

test("PowerShell falls back to a checksummed native mise release", async () => {
  const installer = await readFile(join(repoRoot, "install.ps1"), "utf8");

  assert.match(installer, /RuntimeInformation\]::OSArchitecture/);
  assert.match(installer, /SHASUMS256\.txt/);
  assert.match(installer, /Get-FileHash -Path \$TmpArchive -Algorithm SHA256/);
  assert.match(installer, /mise\\bin\\mise-shim\.exe/);
});

test("PowerShell accepts annotated core manifest markers", async () => {
  const [installer, manifest] = await Promise.all([
    readFile(join(repoRoot, "install.ps1"), "utf8"),
    readFile(join(repoRoot, "manifest", "loom.toml"), "utf8"),
  ]);
  const lines = manifest.split(/\r?\n/);

  assert.equal(lines.includes("# core:begin"), false, "fixture must keep its annotation");
  assert.ok(lines.some((line) => line.startsWith("# core:begin")));
  assert.match(installer, /\.StartsWith\("# core:begin"\)/);
  assert.match(installer, /\.StartsWith\("# core:end"\)/);
});

test("piped Unix bootstrap reconnects interactive setup to the terminal", async () => {
  const installer = await readFile(join(repoRoot, "install.sh"), "utf8");

  assert.match(installer, /exec mise -C "\$HOME" exec -- loom setup "\$@" <\/dev\/tty/);
});

test("bootstraps forward explicit setup selectors", async () => {
  const [unix, windows] = await Promise.all([
    readFile(join(repoRoot, "install.sh"), "utf8"),
    readFile(join(repoRoot, "install.ps1"), "utf8"),
  ]);

  assert.match(unix, /mise -C "\$HOME" exec -- loom setup "\$@"/);
  assert.match(windows, /mise -C \$HOME exec -- loom setup @SetupArgs/);
});

test("bootstraps install only the Loom selection, not the current project", async () => {
  const [unix, windows] = await Promise.all([
    readFile(join(repoRoot, "install.sh"), "utf8"),
    readFile(join(repoRoot, "install.ps1"), "utf8"),
  ]);

  assert.match(unix, /mise -C "\$HOME" install --yes/);
  assert.match(windows, /mise -C \$HOME install --yes/);
});

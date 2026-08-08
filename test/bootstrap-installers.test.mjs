import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");

test("PowerShell refreshes persisted PATH before resolving mise", async () => {
  const installer = await readFile(join(repoRoot, "install.ps1"), "utf8");
  const wingetInstall = installer.indexOf("winget install");
  const machinePath = installer.indexOf("[System.EnvironmentVariableTarget]::Machine");
  const userPath = installer.indexOf("[System.EnvironmentVariableTarget]::User");
  const miseGuard = installer.indexOf(
    "if (-not (Get-Command mise -ErrorAction SilentlyContinue))",
    wingetInstall + 1,
  );

  assert.ok(wingetInstall >= 0, "expected the WinGet install path");
  assert.ok(machinePath > wingetInstall && machinePath < miseGuard);
  assert.ok(userPath > wingetInstall && userPath < miseGuard);
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

  assert.match(installer, /exec mise exec -- loom setup "\$@" <\/dev\/tty/);
});

test("bootstraps forward explicit setup selectors", async () => {
  const [unix, windows] = await Promise.all([
    readFile(join(repoRoot, "install.sh"), "utf8"),
    readFile(join(repoRoot, "install.ps1"), "utf8"),
  ]);

  assert.match(unix, /loom setup "\$@"/);
  assert.match(windows, /loom setup @SetupArgs/);
});

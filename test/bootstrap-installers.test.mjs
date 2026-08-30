import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const unixInstaller = readFile(join(repoRoot, "install.sh"), "utf8");
const windowsInstaller = readFile(join(repoRoot, "install.ps1"), "utf8");

test("PowerShell refreshes persisted PATH before resolving mise", async () => {
  const installer = await windowsInstaller;
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
  const installer = await windowsInstaller;

  assert.match(installer, /RuntimeInformation\]::OSArchitecture/);
  assert.match(installer, /SHASUMS256\.txt/);
  assert.match(installer, /Get-FileHash -Path \$TmpArchive -Algorithm SHA256/);
  assert.match(installer, /mise\\bin\\mise-shim\.exe/);
});

test("PowerShell replaces selections atomically and carries mise ownership across reruns", async () => {
  const installer = await windowsInstaller;

  assert.match(installer, /function Restore-AtomicPath/);
  assert.match(installer, /Restore-AtomicPath \$Selection/);
  assert.match(installer, /Restore-AtomicPath \$PendingMise/);
  assert.match(installer, /Restore-AtomicPath \$ProfilePath/);
  assert.match(installer, /function Set-AtomicLines/);
  assert.match(installer, /\[System\.IO\.File\]::Replace\(\$Incoming, \$Path, \$Backup\)/);
  assert.match(installer, /bootstrap-mise-pending\.json/);
  assert.match(installer, /ConvertFrom-Json/);
  assert.match(installer, /Remove-Item -LiteralPath \$PendingMise/);
});

test("PowerShell accepts annotated core manifest markers", async () => {
  const [installer, manifest] = await Promise.all([
    windowsInstaller,
    readFile(join(repoRoot, "manifest", "loom.toml"), "utf8"),
  ]);
  const lines = manifest.split(/\r?\n/);

  assert.equal(lines.includes("# core:begin"), false, "fixture must keep its annotation");
  assert.ok(lines.some((line) => line.startsWith("# core:begin")));
  assert.match(installer, /\.StartsWith\("# core:begin"\)/);
  assert.match(installer, /\.StartsWith\("# core:end"\)/);
});

test("piped Unix bootstrap reconnects interactive setup to the terminal", async () => {
  const installer = await unixInstaller;

  // The pty behind stderr first: macOS kqueue cannot poll the /dev/tty alias,
  // so a wizard started on it dies with "Failed to initialize input reader".
  assert.match(installer, /terminal="\$\(tty 0<&2 2>\/dev\/null\)"/);
  assert.match(installer, /terminal=\/dev\/tty/);
  assert.match(installer, /run_loom_setup "\$@" <"\$terminal"/);
  // A shell opened before the install has no mise hook: say to open a new one.
  assert.match(installer, /open a new shell \(or run: exec /);
  // A guided install with no terminal at all says what to run instead of
  // launching a UI that cannot read.
  assert.match(installer, /Open a shell and run: loom/);
});

test("bootstraps forward explicit setup selectors", async () => {
  const [unix, windows] = await Promise.all([unixInstaller, windowsInstaller]);

  assert.match(unix, /mise -C "\$HOME" exec -- loom setup "\$@"/);
  assert.match(windows, /mise -C \$HOME exec -- loom setup @SetupArgs/);
});

test("bootstrap CI handoff can exercise the checked-out Loom binary", async () => {
  const [unix, windows] = await Promise.all([unixInstaller, windowsInstaller]);

  assert.match(unix, /LOOM_E2E_LOOM_BIN/);
  assert.match(unix, /"\$LOOM_E2E_LOOM_BIN" setup "\$@"/);
  assert.match(windows, /\$env:LOOM_E2E_LOOM_BIN/);
  assert.match(windows, /& \$env:LOOM_E2E_LOOM_BIN setup @SetupArgs/);
});

test("bootstraps install only the Loom selection, not the current project", async () => {
  const [unix, windows] = await Promise.all([unixInstaller, windowsInstaller]);

  assert.match(unix, /mise -C "\$HOME" install --yes/);
  assert.match(windows, /mise -C \$HOME install --yes/);
});

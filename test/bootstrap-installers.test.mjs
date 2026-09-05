import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { parse } from "yaml";

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
  assert.match(windows, /& \$MiseCommand -C \$HOME exec -- loom setup @SetupArgs/);
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

test("release smoke selects published binaries at the pin commit", async () => {
  const release = parse(await readFile(join(repoRoot, ".github/workflows/release.yml"), "utf8"));
  const smoke = parse(await readFile(join(repoRoot, ".github/workflows/full-install.yml"), "utf8"));
  assert.equal(release.jobs["full-install"].with.published, true);
  assert.equal(release.jobs["full-install"].with.checkout_ref, `\${{ needs.pin.outputs.sha }}`);
  for (const platform of ["unix", "windows"]) {
    const steps = smoke.jobs[platform].steps;
    assert.equal(
      steps.find((step) => step.run?.startsWith("cargo build")).if,
      `\${{ !inputs.published }}`,
    );
    const env = steps.find((step) => step.name === "Run the real bootstrap twice").env;
    assert.match(env.LOOM_E2E_LOOM_BIN, /!inputs\.published.*\|\| ''/);
    assert.match(env.LOOM_REPO_DIR, /!inputs\.published.*\|\| ''/);
    assert.equal(env.LOOM_E2E_PUBLISHED, `\${{ inputs.published }}`);
  }
});

test("Unix smoke checks the published pin rather than an unreleased Cargo version", {
  skip: process.platform === "win32",
}, async () => {
  const source = await readFile(join(repoRoot, "scripts/e2e-install-unix.sh"), "utf8");
  const branch = source.match(/^if \[\[ \$\{LOOM_E2E_PUBLISHED[^\n]+\n[\s\S]*?^fi$/m)?.[0];
  assert.ok(branch, "version selection must exist");
  const workspace = await mkdtemp(join(tmpdir(), "loom-smoke-version-"));
  const manifest = join(workspace, "manifest.toml");
  try {
    await mkdir(join(workspace, "cli/loom"), { recursive: true });
    await writeFile(join(workspace, "cli/loom/Cargo.toml"), 'version = "9.9.9"\n');
    await writeFile(
      manifest,
      '[tools]\n"github:Yassimba/loom[exe=loom]" = { version = "loom-v1.2.3" }\n',
    );
    const version = (published) =>
      execFileSync("bash", ["-eu", "-c", `${branch}\nprintf '%s' "$loom_version"`], {
        encoding: "utf8",
        env: {
          ...process.env,
          workspace,
          manifest,
          LOOM_REPO_DIR: "",
          LOOM_E2E_PUBLISHED: published,
        },
      });
    assert.equal(version("true"), "1.2.3");
    assert.equal(version("false"), "9.9.9");
    await writeFile(manifest, "[tools]\n");
    assert.throws(() => version("true"), /missing published Loom pin/);
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
});

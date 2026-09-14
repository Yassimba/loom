import { join } from "node:path";
import { vol } from "memfs";
import { describe, expect, it } from "vitest";
import { targetsForTool } from "./targets";

describe("targetsForTool", () => {
  it("resolves direct file tool targets from cwd", async () => {
    await expect(targetsForTool("read", { path: "README.md" }, "/repo")).resolves.toEqual([
      { path: "/repo/README.md", unresolved: false, checkPathAccess: true },
    ]);
  });

  it("extracts bash path candidates", async () => {
    const cwd = "/repo";
    vol.fromJSON({ "/repo/README.md": "hello" });

    await expect(targetsForTool("bash", { command: "cat ./README.md" }, cwd)).resolves.toEqual([
      { path: join(cwd, "README.md"), unresolved: false, checkPathAccess: true },
    ]);
  });

  it("extracts paths from PowerShell command strings", async () => {
    const path = "/home/user/.pi/agent/AGENTS.md";
    vol.fromJSON({ [path]: "# global" });

    await expect(
      targetsForTool(
        "bash",
        {
          command: `powershell -Command "Get-Content -Path '${path}' -TotalCount 1"`,
        },
        "/repo",
      ),
    ).resolves.toEqual([{ path, unresolved: false, checkPathAccess: true }]);
  });

  it("extracts paths from Python command strings", async () => {
    const path = "/home/user/.pi/agent/AGENTS.md";
    vol.fromJSON({ [path]: "# global" });

    await expect(
      targetsForTool("bash", { command: `python3 -c 'open("${path}").read()'` }, "/repo"),
    ).resolves.toEqual([{ path, unresolved: false, checkPathAccess: true }]);
  });

  it("does not treat awk regexes as paths", async () => {
    const cwd = "/repo";
    vol.fromJSON({ "/repo/test.txt": "aaa" });

    const targets = await targetsForTool(
      "bash",
      { command: "awk '/aaa/{flag=1} flag{print}' ./test.txt" },
      cwd,
    );
    expect(targets.filter((target) => target.checkPathAccess)).toEqual([
      { path: join(cwd, "test.txt"), unresolved: false, checkPathAccess: true },
    ]);
  });

  it("keeps implausible paths for policies without prompting for path access", async () => {
    await expect(
      targetsForTool("bash", { command: "cat /missing/.env" }, "/repo"),
    ).resolves.toEqual([{ path: "/missing/.env", unresolved: false, checkPathAccess: false }]);
  });

  it("marks shell-variable paths as unresolved", async () => {
    await expect(
      targetsForTool("bash", { command: 'head -c 60 "$SC/.env"' }, "/repo"),
    ).resolves.toContainEqual({
      path: "/repo/$SC/.env",
      unresolved: true,
      checkPathAccess: true,
    });
  });

  it("ignores unrelated tools", async () => {
    await expect(targetsForTool("custom", { path: "README.md" }, "/repo")).resolves.toEqual([]);
  });
});

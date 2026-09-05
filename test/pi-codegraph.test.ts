import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import register from "../plugins/pi-codegraph/index.ts";

test("CodeGraph passes prompts literally, injects context, and tolerates unavailable context", {
  skip: process.platform === "win32",
}, async () => {
  const dir = await mkdtemp(join(tmpdir(), "pi-codegraph-"));
  const previousPath = process.env.PATH;
  try {
    const binary = join(dir, "codegraph");
    await writeFile(binary, '#!/bin/sh\n[ "$1" = prompt-hook ] || exit 1\ncat\n', { mode: 0o755 });
    process.env.PATH = `${dir}:${previousPath}`;
    let handler: (event: { prompt: string }, ctx: { cwd: string }) => Promise<unknown> =
      async () => {
        throw new Error("Hook was not registered");
      };
    register({
      on(event: string, callback: typeof handler) {
        assert.equal(event, "before_agent_start");
        handler = callback;
      },
    } as ExtensionAPI);
    const input = { cwd: dir, prompt: 'Explain "auth" flow; $(never-execute)' };
    assert.deepEqual(await handler(input, { cwd: dir }), {
      message: {
        customType: "codegraph-context",
        content: JSON.stringify({ prompt: input.prompt, cwd: dir }),
        display: false,
      },
    });
    for (const script of ["#!/bin/sh\nexit 0\n", "#!/bin/sh\nexit 1\n"]) {
      await writeFile(binary, script);
      assert.equal(await handler(input, { cwd: dir }), undefined);
    }
    await rm(binary);
    process.env.PATH = dir;
    assert.equal(await handler(input, { cwd: dir }), undefined);
  } finally {
    process.env.PATH = previousPath;
    await rm(dir, { recursive: true, force: true });
  }
});

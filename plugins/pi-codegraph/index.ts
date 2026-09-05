import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

const execute = promisify(execFile);

export default function (pi: ExtensionAPI) {
  pi.on("before_agent_start", async (event, ctx) => {
    try {
      const pending = execute("codegraph", ["prompt-hook"], {
        cwd: ctx.cwd,
        timeout: 10000,
        maxBuffer: 1024 * 1024,
      });
      pending.child.stdin?.on("error", () => {});
      pending.child.stdin?.end(JSON.stringify({ prompt: event.prompt, cwd: ctx.cwd }));
      const { stdout } = await pending;
      const content = stdout.trim();
      if (content) {
        return {
          message: { customType: "codegraph-context", content, display: false },
        };
      }
    } catch {
      // Optional context must never prevent submitting a prompt.
    }
  });
}

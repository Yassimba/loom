import type { Register } from "claude-code";

const ARGV = ["loom", "wiki", "sessions", "--quiet"];

// The sync runs at session start, in the background. A mod's child process
// ends with the session, and session.end has only a short time budget, so a
// session reaches the Vault when the next session starts.
export const register: Register = (on) => {
  on("session.start", async ($, e, next) => {
    const started = await next(e);
    // Skip `claude -p` runs and the SDK: scripts would start a sync each call.
    if (!e.isInteractive) return started;
    void (async () => {
      try {
        for await (const _ of $.process.spawn({ argv: ARGV })) {
          // Drain the output. `loom` reports nothing with --quiet.
        }
      } catch {
        // `loom` is not installed or no Vault has sessions on: nothing to do.
      }
    })();
    return started;
  });
};

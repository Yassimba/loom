import { expect, test } from "claude-code/testing";

// Capability: loom-sessions starts one background session sync per session.
// Seam: the engine's session.start and process.spawn events, with the test's
// hooks in place of the engine beneath the mod.
// Fails when: the mod does not start `loom wiki sessions --quiet` at an
// interactive session start, starts it for a `-p` run, or a missing `loom`
// binary breaks the session start.

test("session start runs the quiet session sync", async ($, on) => {
  on("session.start", (_, e) => ({ cwd: e.cwd }));
  const spawned: string[][] = [];
  on("process.spawn", async function* (_, e) {
    spawned.push(e.argv);
    yield { stream: "stderr" as const, text: "Amp thread T-1 skipped: offline\n" };
    return { value: { code: 0, signal: null } };
  });
  await $.session.start({ cwd: "/tmp", surface: "terminal", isInteractive: true });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(spawned).toEqual([["loom", "wiki", "sessions", "--quiet"]]);
  await $.session.start({ cwd: "/tmp", surface: null, isInteractive: false });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(spawned).toHaveLength(1);
});

test("a failed spawn does not fail the session start", async ($, on) => {
  on("session.start", (_, e) => ({ cwd: e.cwd }));
  // No process.spawn hook answers, so the spawn rejects as if `loom` were missing.
  await expect(
    $.session.start({ cwd: "/tmp", surface: "terminal", isInteractive: true }),
  ).resolves.toEqual({ cwd: "/tmp" });
});

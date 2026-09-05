import assert from "node:assert/strict";
import test from "node:test";

// Pi 0.85.0 omits pi-server from its SDK runtime dependencies. Remove Loom's
// direct pi-server dependency once upstream declares it and this passes after npm ci.
test("Pi SDK entrypoint loads with its runtime dependencies", async () => {
  const sdk = await import("@earendil-works/pi-coding-agent");
  assert.equal(typeof sdk.createAgentSession, "function");
});

import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test, { afterEach, beforeEach } from "node:test";
import { setImmediate } from "node:timers/promises";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import { visibleWidth } from "@earendil-works/pi-tui";
import register from "../plugins/pi-loom/index.ts";
import {
  availableLoomVersion,
  checkLoomUpdate,
  execVersionCommand,
} from "../plugins/pi-loom/src/update.ts";

let previousOffline: string | undefined;
beforeEach(() => {
  previousOffline = process.env.PI_OFFLINE;
  process.env.PI_OFFLINE = "";
});
afterEach(() => {
  if (previousOffline === undefined) delete process.env.PI_OFFLINE;
  else process.env.PI_OFFLINE = previousOffline;
});

const manifest = (version: string) =>
  `[tools]\n"github:Yassimba/loom[exe=loom]" = { version = "loom-v${version}", tag_regex = "^loom-v" }\n`;
const installed: ExtensionAPI["exec"] = async () => ({
  stdout: "loom 0.19.1\n",
  stderr: "",
  code: 0,
  killed: false,
});

test("availableLoomVersion compares the exact manifest pin numerically", async () => {
  for (const [current, published, expected] of [
    ["0.19.1", "0.19.1", undefined],
    ["0.19.2", "0.19.1", undefined],
    ["0.19.1", "0.19.2", "0.19.2"],
    ["0.9.9", "0.19.1", "0.19.1"],
    ["0.19.9", "1.0.0", "1.0.0"],
    ["1.0.0", "0.99.99", undefined],
    ["0.19.1-dev", "0.19.2", undefined],
    ["0.19.1", "0.20.0-rc.1", undefined],
    ["00.19.1", "0.20.0", undefined],
  ] as const) {
    assert.equal(availableLoomVersion(`loom ${current}\n`, manifest(published)), expected);
  }
  const published = await readFile(new URL("../manifest/loom.toml", import.meta.url), "utf8");
  assert.match(availableLoomVersion("loom 0.0.0", published) ?? "", /^\d+\.\d+\.\d+$/);
  for (const invalid of [
    "not TOML",
    manifest("9.0.0").replace("[tools]", "[other]"),
    manifest("9.0.0").replace("exe=loom]", "exe=loom-teams]"),
    manifest("9.0.0").replace('"github:', '# "github:'),
    manifest("9.0.0").replace("loom-v9.0.0", "pi-loom-v9.0.0"),
    `[tools]\n[other]\n${manifest("9.0.0").split("\n").slice(1).join("\n")}`,
  ]) {
    assert.equal(availableLoomVersion("loom 0.19.1", invalid), undefined);
  }
  assert.equal(availableLoomVersion("pi 0.19.1", manifest("9.0.0")), undefined);
});

test("version probe kills a subprocess that ignores SIGTERM", async () => {
  const directory = await mkdtemp(join(tmpdir(), "loom-version-timeout-"));
  const pidFile = join(directory, "pid");
  let deadline: ReturnType<typeof setTimeout> | undefined;
  try {
    const probe = execVersionCommand(
      process.execPath,
      [
        "-e",
        `
      require('fs').writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));
      process.on('SIGTERM', () => {});
      setInterval(() => {}, 1000);
    `,
      ],
      { timeout: 1500 },
    );
    await assert.rejects(
      Promise.race([
        probe,
        new Promise((_, reject) => {
          deadline = setTimeout(() => reject(new Error("probe exceeded deadline")), 5000);
        }),
      ]),
      { signal: "SIGKILL" },
    );
    const pid = Number(await readFile(pidFile, "utf8"));
    assert.throws(() => process.kill(pid, 0), { code: "ESRCH" });
  } finally {
    clearTimeout(deadline);
    try {
      process.kill(Number(await readFile(pidFile, "utf8")), "SIGKILL");
    } catch {}
    await rm(directory, { recursive: true, force: true });
  }
});

test("checkLoomUpdate uses bounded execution and the published main manifest", async (t) => {
  process.env.PI_OFFLINE = "0";
  const fetchMock = t.mock.method(
    globalThis,
    "fetch",
    async (url: string | URL | Request, options?: RequestInit) => {
      assert.equal(url, "https://raw.githubusercontent.com/Yassimba/loom/main/manifest/loom.toml");
      assert.ok(options?.signal instanceof AbortSignal);
      return new Response(manifest("0.20.0"));
    },
  );
  const exec: ExtensionAPI["exec"] = async (command, args, options) => {
    assert.equal(command, "loom");
    assert.deepEqual(args, ["--version"]);
    assert.equal(options?.timeout, 1500);
    assert.ok(options?.signal instanceof AbortSignal);
    return installed(command, args, options);
  };
  assert.equal(await checkLoomUpdate(exec, new AbortController().signal), "0.20.0");
  assert.equal(fetchMock.mock.callCount(), 1);
});

test("checkLoomUpdate silently skips offline, missing, killed, malformed and failed checks", async (t) => {
  const fetchMock = t.mock.method(
    globalThis,
    "fetch",
    async () => new Response("", { status: 503 }),
  );
  for (const value of ["1", "true", "YES"]) {
    process.env.PI_OFFLINE = value;
    assert.equal(
      await checkLoomUpdate(async () => assert.fail("offline exec"), new AbortController().signal),
      undefined,
    );
  }
  process.env.PI_OFFLINE = "";
  for (const exec of [
    async () => {
      throw new Error("ENOENT");
    },
    async () => ({ stdout: "", stderr: "", code: 1, killed: false }),
    async () => ({ stdout: "loom 0.19.1", stderr: "", code: 0, killed: true }),
    async () => ({ stdout: "unknown", stderr: "", code: 0, killed: false }),
  ]) {
    assert.equal(await checkLoomUpdate(exec, new AbortController().signal), undefined);
  }
  assert.equal(fetchMock.mock.callCount(), 0);
  assert.equal(await checkLoomUpdate(installed, new AbortController().signal), undefined);
  fetchMock.mock.mockImplementation(async () => {
    throw new Error("network timeout");
  });
  assert.equal(await checkLoomUpdate(installed, new AbortController().signal), undefined);
});

test("checkLoomUpdate aborts network work at its overall deadline", async (t) => {
  const deadline = new AbortController();
  t.mock.method(AbortSignal, "timeout", (milliseconds: number) => {
    assert.equal(milliseconds, 4000);
    return deadline.signal;
  });
  let networkSignal: AbortSignal | undefined;
  t.mock.method(globalThis, "fetch", (_url: string | URL | Request, options?: RequestInit) => {
    assert.ok(options?.signal instanceof AbortSignal);
    networkSignal = options.signal;
    return new Promise<Response>((_resolve, reject) => {
      networkSignal?.addEventListener("abort", () => reject(new Error("timeout")), { once: true });
    });
  });
  const pending = checkLoomUpdate(installed, new AbortController().signal);
  await setImmediate();
  deadline.abort();
  assert.equal(await pending, undefined);
  assert.equal(networkSignal?.aborted, true);
});

type Handler = (event: { reason: string }, context: ExtensionContext) => unknown;
function runtime(exec: ExtensionAPI["exec"] = installed) {
  const handlers = new Map<string, Handler>();
  const notices: string[] = [];
  const headers: Parameters<ExtensionContext["ui"]["setHeader"]>[0][] = [];
  const ctx = {
    mode: "tui",
    hasUI: true,
    ui: {
      setHeader: (header: (typeof headers)[number]) => headers.push(header),
      notify: (message: string) => notices.push(message),
    },
  } as unknown as ExtensionContext;
  register(
    {
      on: (event: string, handler: Handler) => handlers.set(event, handler),
    } as unknown as ExtensionAPI,
    exec,
  );
  const start = handlers.get("session_start");
  const shutdown = handlers.get("session_shutdown");
  assert.ok(start && shutdown);
  return {
    notices,
    headers,
    ctx,
    start: (reason = "startup") => start({ reason }, ctx),
    shutdown: () => shutdown({ reason: "quit" }, ctx),
  };
}

test("pi-loom starts without waiting, checks once, and renders a width-safe header", async (t) => {
  process.env.PI_OFFLINE = "";
  t.mock.method(globalThis, "fetch", async () => new Response(manifest("0.20.0")));
  let complete!: (value: Awaited<ReturnType<typeof installed>>) => void;
  let calls = 0;
  const app = runtime(() => {
    calls++;
    return new Promise((resolve) => {
      complete = resolve;
    });
  });
  assert.equal(app.start(), undefined);
  assert.equal(calls, 1);
  app.start();
  assert.equal(calls, 1);
  assert.deepEqual(app.notices, []);
  complete(await installed("loom", ["--version"]));
  await setImmediate();
  assert.deepEqual(app.notices, [
    "Loom update available: 0.20.0. Run loom update or use the /loom skill to update.",
  ]);
  const theme = {
    bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
    fg: (_: string, s: string) => `\x1b[35m${s}\x1b[0m`,
  };
  const createHeader = app.headers[0];
  assert.ok(createHeader);
  const header = createHeader({} as never, theme as never);
  for (const width of [0, 1, 4, 20, 47, 48, 80, 120]) {
    assert.ok(header.render(width).every((line) => visibleWidth(line) <= width));
  }
  assert.match(header.render(80)[0], /⢠⣴⣦⡀.*Loom/);
  assert.ok(header.render(80)[0].startsWith("\x1b[38;2;128;56;233m"));
  assert.match(header.render(80)[1], /Your opinionated agent setup\./);
  assert.match(header.render(80)[3], /\/loom.*Set up, add tools, or update\./);
  assert.doesNotMatch(header.render(80).join("\n"), /\/help/);
  header.invalidate();
  assert.equal(header.render(20).length, 1);
  app.shutdown();
  // Pi constructs a fresh extension instance for these events.
  for (const reason of ["reload", "new", "resume", "fork"]) {
    const replacement = runtime(async () => assert.fail("repeated check"));
    replacement.start(reason);
    assert.equal(replacement.headers.length, 1);
  }
});

test("pi-loom has no headless UI or startup work", () => {
  for (const mode of ["rpc", "print", "json"] as const) {
    const app = runtime(async () => assert.fail("headless check"));
    app.ctx.mode = mode;
    app.start();
    assert.deepEqual(app.headers, []);
    assert.deepEqual(app.notices, []);
  }
});

test("pi-loom cancels in-flight work and never notifies a shut-down session", async (t) => {
  process.env.PI_OFFLINE = "";
  let finish!: (response: Response) => void;
  let signal!: AbortSignal;
  t.mock.method(globalThis, "fetch", (_url: string | URL | Request, options?: RequestInit) => {
    assert.ok(options?.signal instanceof AbortSignal);
    signal = options.signal;
    return new Promise<Response>((resolve) => {
      finish = resolve;
    });
  });
  const app = runtime();
  app.start();
  await setImmediate();
  app.shutdown();
  assert.equal(signal.aborted, true);
  finish(new Response(manifest("0.20.0")));
  await setImmediate();
  assert.deepEqual(app.notices, []);
});

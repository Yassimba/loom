import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, sep } from "node:path";
import { test } from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import type { AutocompleteProvider } from "@earendil-works/pi-tui";
import piAddDirPrototype, {
  completeDirectories,
  PERMISSION_CHOICES,
  shouldSubmitAddDirPath,
} from "../plugins/pi-add-dir-prototype/src/index.ts";

test("dot-dot immediately completes sibling directories before the trailing slash", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const current = join(root, "current");
  await mkdir(current);
  await mkdir(join(root, "sibling"));

  try {
    const completions = completeDirectories("..", current) ?? [];
    assert(completions.some((item) => item.value === `..${sep}sibling${sep}`));
  } finally {
    await rm(root, { recursive: true });
  }
});

test("zoxide frecency sorts directories within a relative parent", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const current = join(root, "current");
  const low = join(root, "alphabetical-first");
  const high = join(root, "frequent");
  await mkdir(current);
  await mkdir(low);
  await mkdir(high);

  try {
    const completions = completeDirectories("..", current, [high, low]) ?? [];
    assert.equal(completions[0]?.value, `..${sep}frequent${sep}`);
    assert.equal(completions[1]?.value, `..${sep}alphabetical-first${sep}`);
  } finally {
    await rm(root, { recursive: true });
  }
});

test("a second forced Tab still uses add-dir command completion", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  await mkdir(join(root, "child"));
  let wrapProvider: ((current: AutocompleteProvider) => AutocompleteProvider) | undefined;
  type FakeContext = {
    cwd: string;
    sessionManager: { getBranch: () => never[] };
    ui: {
      setWidget: () => void;
      setEditorComponent: (factory: unknown) => void;
      addAutocompleteProvider: (
        wrapper: (current: AutocompleteProvider) => AutocompleteProvider,
      ) => void;
    };
  };
  let sessionStart: ((event: unknown, ctx: FakeContext) => Promise<void>) | undefined;
  const pi = {
    on(name: string, handler: unknown) {
      if (name === "session_start") {
        sessionStart = handler as (event: unknown, ctx: FakeContext) => Promise<void>;
      }
    },
    registerCommand() {},
    appendEntry() {},
    exec: async () => ({ code: 0, stdout: "", stderr: "" }),
  };
  piAddDirPrototype(pi as unknown as ExtensionAPI);

  try {
    assert(sessionStart);
    await sessionStart(
      {},
      {
        cwd: root,
        sessionManager: { getBranch: () => [] },
        ui: {
          setWidget() {},
          setEditorComponent() {},
          addAutocompleteProvider(wrapper) {
            wrapProvider = wrapper;
          },
        },
      },
    );
    const current = {
      async getSuggestions() {
        return null;
      },
      applyCompletion() {
        throw new Error("not used");
      },
    };
    assert(wrapProvider);
    const provider = wrapProvider(current);
    const line = "/add-dir ./";
    const suggestions = await provider?.getSuggestions([line], 0, line.length, {
      force: true,
      signal: new AbortController().signal,
    });
    assert(suggestions?.items.some((item) => item.value === `.${sep}child${sep}`));
  } finally {
    await rm(root, { recursive: true });
  }
});

test("Enter submits the current add-dir path instead of its highlighted child", () => {
  assert.equal(shouldSubmitAddDirPath("\r", "/add-dir ../../enexis/turbine/", true), true);
  assert.equal(shouldSubmitAddDirPath("\t", "/add-dir ../../enexis/turbine/", true), false);
});

test("one permission prompt includes every access and lifetime combination", () => {
  assert.deepEqual(
    PERMISSION_CHOICES.map(({ access, scope }) => `${access}:${scope}`),
    [
      "read:session",
      "write:session",
      "read:project",
      "write:project",
      "read:global",
      "write:global",
    ],
  );
});

test("nested dot-dot immediately completes directories at any parent depth", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const current = join(root, "ancestor", "parent", "current");
  await mkdir(current, { recursive: true });
  await mkdir(join(root, "target"));

  try {
    const completions = completeDirectories(join("..", "..", ".."), current) ?? [];
    assert(completions.some((item) => item.value === `${join("..", "..", "..", "target")}${sep}`));
  } finally {
    await rm(root, { recursive: true });
  }
});

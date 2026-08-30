import assert from "node:assert/strict";
import { realpathSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, sep } from "node:path";
import { test } from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import type { AutocompleteProvider } from "@earendil-works/pi-tui";
import piAddDir, {
  completeAddedDirectories,
  completeDirectories,
  footerStatus,
  matchAddedDirectory,
  shouldSubmitAddDirPath,
} from "../plugins/pi-add-dir/src/index.ts";
import {
  directoryConfigPath,
  loadDirectories,
  saveDirectories,
} from "../plugins/pi-add-dir/src/storage.ts";

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
    isProjectTrusted: () => boolean;
    ui: {
      setStatus: () => void;
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
  piAddDir(pi as unknown as ExtensionAPI);

  try {
    assert(sessionStart);
    await sessionStart(
      {},
      {
        cwd: root,
        sessionManager: { getBranch: () => [] },
        isProjectTrusted: () => false,
        ui: {
          setStatus() {},
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
  assert.equal(shouldSubmitAddDirPath("\r", "/add-dir ../../projects/turbine/", true), true);
  assert.equal(shouldSubmitAddDirPath("\t", "/add-dir ../../projects/turbine/", true), false);
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

test("footer status clearly labels added directories", () => {
  assert.equal(footerStatus([]), undefined);
  assert.equal(footerStatus(["/tmp/turbine"]), "added dirs turbine");
  assert.equal(footerStatus(["/tmp/turbine", "/tmp/loom"]), "added dirs turbine, loom");
});

test("matchAddedDirectory accepts a basename or a real path", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const turbine = join(root, "turbine");
  await mkdir(turbine);

  try {
    const resolved = realpathSync(turbine);
    const directories = [resolved];
    assert.equal(matchAddedDirectory(directories, "turbine", root), resolved);
    assert.equal(matchAddedDirectory(directories, turbine, root), resolved);
    assert.equal(matchAddedDirectory(directories, "missing", root), undefined);
  } finally {
    await rm(root, { recursive: true });
  }
});

test("rm-dir completion lists only added directories", () => {
  const directories = ["/tmp/turbine", "/tmp/loom"];
  const items = completeAddedDirectories("tur", directories) ?? [];
  assert.equal(items.length, 1);
  assert.equal(items[0]?.value, "/tmp/turbine");
  assert.equal(items[0]?.label, "turbine");
});

test("project directories persist outside the session", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  try {
    assert.deepEqual(saveDirectories("project", root, ["/tmp/turbine"]), { ok: true });
    assert.deepEqual(loadDirectories("project", root), { directories: ["/tmp/turbine"] });
  } finally {
    await rm(root, { recursive: true });
  }
});

test("saving refuses to replace malformed directory config", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const path = directoryConfigPath("project", root);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, "not json", "utf8");
  try {
    const result = saveDirectories("project", root, ["/tmp/turbine"]);
    assert.equal(result.ok, false);
  } finally {
    await rm(root, { recursive: true });
  }
});

test("global scope warns that instructions affect every project", async () => {
  const root = await mkdtemp(join(tmpdir(), "pi-add-dir-"));
  const external = join(root, "external");
  await mkdir(external);
  let addDir: ((args: string, ctx: unknown) => Promise<void>) | undefined;
  let scopePrompt: { title: string; options: string[] } | undefined;
  let confirmation: { title: string; message: string } | undefined;
  const pi = {
    on() {},
    appendEntry() {},
    exec: async () => ({ code: 0, stdout: "", stderr: "" }),
    registerCommand(name: string, command: { handler: typeof addDir }) {
      if (name === "add-dir") addDir = command.handler;
    },
  };
  piAddDir(pi as unknown as ExtensionAPI);

  try {
    assert(addDir);
    await addDir(external, {
      cwd: root,
      hasUI: true,
      isProjectTrusted: () => true,
      ui: {
        async select(title: string, options: string[]) {
          scopePrompt = { title, options };
          return "All projects (global)";
        },
        async confirm(title: string, message: string) {
          confirmation = { title, message };
          return false;
        },
        notify() {},
      },
    });
    assert.equal(scopePrompt?.title, "Add directory for:");
    assert.deepEqual(scopePrompt?.options, [
      "This session",
      "This project",
      "All projects (global)",
    ]);
    assert.equal(confirmation?.title, "Add directory globally?");
    assert.match(confirmation?.message ?? "", /instructions will be added to every project/);
  } finally {
    await rm(root, { recursive: true });
  }
});

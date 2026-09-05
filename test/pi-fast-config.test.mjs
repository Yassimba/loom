import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  configPaths,
  DEFAULT_FAST_CONFIG,
  loadConfig,
  normalizeFastColorValue,
  resolveFastColorValue,
  saveDesiredActive,
} from "../plugins/pi-fast/src/config.ts";

// configPaths derives the global path from os.homedir(), which honors $HOME.
async function withTempHome(run) {
  const home = await mkdtemp(join(tmpdir(), "pi-fast-config-"));
  const previousHome = process.env.HOME;
  process.env.HOME = home;
  try {
    return await run({ home, cwd: join(home, "repo") });
  } finally {
    process.env.HOME = previousHome;
  }
}

async function writeConfigFile(path, value) {
  await mkdir(join(path, ".."), { recursive: true });
  await writeFile(path, typeof value === "string" ? value : JSON.stringify(value), "utf8");
}

test("default config allows future models on supported providers", () => {
  assert.deepEqual(DEFAULT_FAST_CONFIG, {
    persistState: false,
    desiredActive: false,
    supportedModels: ["openai/*", "openai-codex/*", "xai/*"],
    footer: { mode: "replace", vars: {} },
  });
});

test("uses the required global and project config paths", async () => {
  await withTempHome(async ({ home, cwd }) => {
    assert.deepEqual(configPaths(cwd), {
      project: join(cwd, ".pi", "extensions", "pi-fast.json"),
      global: join(home, ".pi", "agent", "extensions", "pi-fast.json"),
    });
  });
});

test("creates global defaults when no config file exists", async () => {
  await withTempHome(async ({ cwd }) => {
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config, DEFAULT_FAST_CONFIG);
    assert.deepEqual(warnings, []);
    const written = JSON.parse(await readFile(configPaths(cwd).global, "utf8"));
    assert.deepEqual(written, {
      persistState: false,
      desiredActive: false,
      supportedModels: DEFAULT_FAST_CONFIG.supportedModels,
      footer: { mode: "replace", vars: {} },
    });
  });
});

test("project config overrides global config field by field", async () => {
  await withTempHome(async ({ cwd }) => {
    const paths = configPaths(cwd);
    await writeConfigFile(paths.global, {
      persistState: true,
      desiredActive: true,
      supportedModels: ["partner/gpt-5.5"],
      footer: { mode: "status" },
    });
    await writeConfigFile(paths.project, { footer: { mode: "off" } });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(warnings, []);
    assert.equal(config.persistState, true);
    assert.equal(config.desiredActive, true);
    assert.deepEqual(config.supportedModels, ["partner/gpt-5.5"]);
    assert.equal(config.footer.mode, "off");
  });
});

test("trims supported model entries and drops invalid ones with warnings", async () => {
  await withTempHome(async ({ cwd }) => {
    const path = configPaths(cwd).global;
    await writeConfigFile(path, {
      supportedModels: [" openai/gpt-5.5 ", "", "missing-slash", "openai/gpt-*", 123],
    });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config.supportedModels, ["openai/gpt-5.5"]);
    assert.equal(warnings.length, 1);
    assert.match(warnings[0], /Ignoring these supportedModels entries/);
    assert.match(warnings[0], /missing-slash/);
  });
});

test("provider wildcards survive loading and preference saves; other patterns are rejected", async () => {
  await withTempHome(async ({ cwd }) => {
    const path = configPaths(cwd).global;
    await writeConfigFile(path, {
      supportedModels: [
        " openai/* ",
        "partner/exact",
        "*/*",
        "openai*/model",
        "openai/**",
        "openai/nested/*",
      ],
    });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config.supportedModels, ["openai/*", "partner/exact"]);
    assert.equal(warnings.length, 1);
    assert.equal((await saveDesiredActive(cwd, true)).ok, true);
    const written = JSON.parse(await readFile(path, "utf8"));
    assert.deepEqual(written.supportedModels, config.supportedModels);
  });
});

test("warns when supportedModels is not an array and keeps the base list", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, { supportedModels: "openai/gpt-5.5" });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config.supportedModels, DEFAULT_FAST_CONFIG.supportedModels);
    assert.match(warnings[0], /Use an array such as/);
  });
});

test("warns when every supportedModels entry is invalid", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, { supportedModels: ["nope", ""] });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config.supportedModels, []);
    assert.equal(warnings.length, 2);
    assert.match(warnings[1], /None of the supportedModels entries .* are valid/);
  });
});

test("warns and uses defaults for an unreadable config layer", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, "{not json");
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(config, DEFAULT_FAST_CONFIG);
    assert.equal(warnings.length, 1);
    assert.match(warnings[0], /Couldn't load .* contains a JSON object/);
  });
});

test("accepts valid footer colors and variable references", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, {
      footer: {
        mode: "replace",
        vars: { brand: "#00ffaa" },
        darkFastColor: "brand",
        lightFastColor: 33,
      },
    });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(warnings, []);
    assert.equal(config.footer.darkFastColor, "brand");
    assert.equal(config.footer.lightFastColor, 33);
  });
});

test("warns on invalid, missing-variable, and circular color tokens", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, {
      footer: {
        vars: { loop: "loop" },
        darkFastColor: "#12345", // malformed hex
        lightFastColor: "loop",
      },
    });
    const { config, warnings } = await loadConfig(cwd);
    assert.equal(config.footer.darkFastColor, undefined);
    assert.equal(config.footer.lightFastColor, undefined);
    assert.equal(warnings.length, 2);
    assert.match(warnings[0], /footer\.darkFastColor=.*use a hex color/);
    assert.match(warnings[1], /footer\.lightFastColor=.*refers back to itself/);

    await writeConfigFile(configPaths(cwd).global, { footer: { darkFastColor: "missingVar" } });
    const second = await loadConfig(cwd);
    assert.match(second.warnings[0], /define color variable "missingVar" in footer.vars/);
  });
});

test("accepts colors that older releases treated as legacy defaults", async () => {
  await withTempHome(async ({ cwd }) => {
    await writeConfigFile(configPaths(cwd).global, {
      footer: { darkFastColor: "#FF50BE", lightFastColor: "#d20000" },
    });
    const { config, warnings } = await loadConfig(cwd);
    assert.deepEqual(warnings, []);
    assert.equal(config.footer.darkFastColor, "#FF50BE");
    assert.equal(config.footer.lightFastColor, "#d20000");
  });
});

test("normalizes color tokens", () => {
  assert.equal(normalizeFastColorValue("#00ffAA"), "#00ffAA");
  assert.equal(normalizeFastColorValue(" 42 "), "42");
  assert.equal(normalizeFastColorValue(42), 42);
  assert.equal(normalizeFastColorValue(300), undefined);
  assert.equal(normalizeFastColorValue("300"), undefined);
  assert.equal(normalizeFastColorValue(""), "");
  assert.equal(normalizeFastColorValue("someVar"), "someVar");
  assert.equal(normalizeFastColorValue("not a var"), undefined);
  assert.equal(normalizeFastColorValue(null), undefined);
});

test("resolves nested variable chains to concrete tokens", () => {
  const vars = { a: "b", b: "#112233" };
  assert.deepEqual(resolveFastColorValue("a", vars), { value: "#112233" });
  assert.deepEqual(resolveFastColorValue("#445566", {}), { value: "#445566" });
  assert.match(resolveFastColorValue("ghost", {}).error, /define color variable "ghost"/);
  assert.match(resolveFastColorValue("x", { x: "y", y: "x" }).error, /refers back to itself/);
});

test("saves desiredActive to the global config by default", async () => {
  await withTempHome(async ({ cwd }) => {
    const paths = configPaths(cwd);
    await writeConfigFile(paths.global, { persistState: true, customField: "kept" });
    const result = await saveDesiredActive(cwd, true);
    assert.equal(result.ok, true);
    assert.deepEqual(result.warnings, []);
    const written = JSON.parse(await readFile(paths.global, "utf8"));
    assert.equal(written.desiredActive, true);
    assert.equal(written.persistState, true);
    assert.equal(written.customField, "kept", "unknown fields survive the rewrite");
  });
});

test("saves desiredActive to the project config when one exists", async () => {
  await withTempHome(async ({ cwd }) => {
    const paths = configPaths(cwd);
    await writeConfigFile(paths.global, { persistState: true });
    await writeConfigFile(paths.project, { footer: { mode: "status" } });
    assert.equal((await saveDesiredActive(cwd, true)).ok, true);
    const project = JSON.parse(await readFile(paths.project, "utf8"));
    assert.equal(project.desiredActive, true);
    assert.equal(project.footer.mode, "status");
    const global = JSON.parse(await readFile(paths.global, "utf8"));
    assert.equal(global.desiredActive, undefined, "global config left untouched");
  });
});

test("save sanitizes invalid fields", async () => {
  await withTempHome(async ({ cwd }) => {
    const path = configPaths(cwd).global;
    await writeConfigFile(path, {
      persistState: "yes",
      supportedModels: ["openai/gpt-5.5", "bad entry"],
      footer: { mode: "banner", vars: { ok: "#112233", bad: 7 }, darkFastColor: "#12345" },
    });
    const result = await saveDesiredActive(cwd, false);
    assert.equal(result.ok, true);
    assert.equal(result.warnings.length, 2); // dropped model entry + invalid color
    const written = JSON.parse(await readFile(path, "utf8"));
    assert.deepEqual(written, {
      desiredActive: false,
      supportedModels: ["openai/gpt-5.5"],
      footer: { vars: { ok: "#112233" } },
    });
  });
});

test("save creates defaults when no config exists yet", async () => {
  await withTempHome(async ({ cwd }) => {
    assert.equal((await saveDesiredActive(cwd, true)).ok, true);
    const written = JSON.parse(await readFile(configPaths(cwd).global, "utf8"));
    assert.equal(written.desiredActive, true);
    assert.deepEqual(written.supportedModels, DEFAULT_FAST_CONFIG.supportedModels);
  });
});

test("save refuses to clobber a malformed config file", async () => {
  await withTempHome(async ({ cwd }) => {
    const path = configPaths(cwd).global;
    await writeConfigFile(path, "{broken");
    const result = await saveDesiredActive(cwd, true);
    assert.equal(result.ok, false);
    assert.match(result.warnings[0], /contains a JSON object\. The file was left unchanged/);
    assert.equal(await readFile(path, "utf8"), "{broken", "malformed file left untouched");
  });
});

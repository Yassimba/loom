import assert from "node:assert/strict";
import { mkdir, mkdtemp, readdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { buildSetupCatalog, buildSetupCatalogDocument } from "../scripts/catalog-lib.mjs";

async function createCatalogFixture() {
  const repoRoot = await mkdtemp(join(tmpdir(), "pi-catalog-test-"));
  await mkdir(join(repoRoot, "skills", "reviewed"), {
    recursive: true,
  });
  await mkdir(join(repoRoot, "skills", "helper"), {
    recursive: true,
  });
  await mkdir(join(repoRoot, "personal", "private"), {
    recursive: true,
  });
  await mkdir(join(repoRoot, "plugins", "sample"), { recursive: true });
  await mkdir(join(repoRoot, "plugins", "herdr-sample"), { recursive: true });
  await writeFile(
    join(repoRoot, "skills.sh.json"),
    JSON.stringify({ groupings: [{ title: "Coding", skills: ["reviewed", "helper"] }] }),
  );
  await writeFile(
    join(repoRoot, "skills", "reviewed", "SKILL.md"),
    "---\nname: reviewed\ndescription: A reviewed skill\n---\n# Reviewed\n",
  );
  await writeFile(join(repoRoot, "skills", "reviewed", "deps.yml"), "skills:\n  - helper\n");
  await writeFile(
    join(repoRoot, "skills", "helper", "SKILL.md"),
    "---\nname: helper\ndescription: A helper skill\n---\n# Helper\n",
  );
  await writeFile(
    join(repoRoot, "personal", "private", "SKILL.md"),
    "---\nname: private\ndescription: Private\n---\n# Private\n",
  );
  await writeFile(
    join(repoRoot, "plugins", "sample", "package.json"),
    JSON.stringify({
      name: "@example/pi-sample",
      description: "Package fallback",
      loom: {
        catalog: {
          label: "Sample",
          description: "Sample Pi package",
          nextAction: "Run Pi",
        },
      },
    }),
  );
  await writeFile(
    join(repoRoot, "plugins", "herdr-sample", "herdr-plugin.toml"),
    'id = "example.sample"\nname = "Sample Herdr plugin"\nversion = "1.0.0"\ndescription = "Sample Herdr capability"\n',
  );
  await mkdir(join(repoRoot, "manifest"), { recursive: true });
  await writeFile(
    join(repoRoot, "manifest", "herdr-plugins.json"),
    JSON.stringify({
      plugins: [
        {
          id: "remote",
          label: "Remote plugin",
          description: "Hosted elsewhere",
          installTarget: "example/herdr-remote",
          nextAction: "Bind a key.",
        },
      ],
    }),
  );
  await writeFile(
    join(repoRoot, "manifest", "loom.toml"),
    '[tools]\n# core:begin\nnode = "24.19.0"\n# core:end\n\ngh = "2.97.0"\n',
  );
  await writeFile(
    join(repoRoot, "manifest", "tools.json"),
    JSON.stringify({
      tools: [
        {
          key: "gh",
          label: "gh",
          description: "GitHub CLI",
          nextAction: "Run `gh auth login` once.",
        },
      ],
    }),
  );
  await writeFile(
    join(repoRoot, "manifest", "profiles.json"),
    JSON.stringify({
      profiles: [
        {
          id: "engineer",
          label: "Engineer",
          description: "Build and review software.",
          resources: ["skill:reviewed", "tool:gh"],
        },
      ],
    }),
  );
  return repoRoot;
}

test("the setup catalog carries ordered profiles with exact resource ids", async () => {
  const repoRoot = await createCatalogFixture();

  const catalog = await buildSetupCatalogDocument(repoRoot);

  assert.deepEqual(catalog.profiles, [
    {
      id: "engineer",
      label: "Engineer",
      description: "Build and review software.",
      resources: ["skill:reviewed", "tool:gh"],
    },
  ]);
  assert.equal(
    catalog.resources.some(({ id }) => id === "skill:reviewed"),
    true,
  );
});

test("the setup catalog combines opted-in extensions with reviewed skills", async () => {
  const repoRoot = await createCatalogFixture();

  const catalog = await buildSetupCatalog(repoRoot);

  assert.deepEqual(catalog, [
    {
      id: "pi-package:@example/pi-sample",
      kind: "pi-package",
      group: "Pi packages",
      label: "Sample",
      description: "Sample Pi package",
      installTarget: "@example/pi-sample",
      nextAction: "Run Pi",
    },
    {
      id: "skill:reviewed",
      kind: "skill",
      group: "Coding",
      label: "reviewed",
      description: "A reviewed skill",
      installTarget: "reviewed",
      nextAction: "Ask your coding agent to use the reviewed skill.",
      dependencies: ["helper"],
    },
    {
      id: "skill:helper",
      kind: "skill",
      group: "Coding",
      label: "helper",
      description: "A helper skill",
      installTarget: "helper",
      nextAction: "Ask your coding agent to use the helper skill.",
      dependencies: [],
    },
    {
      id: "herdr-plugin:remote",
      kind: "herdr-plugin",
      group: "Herdr plugins",
      label: "Remote plugin",
      description: "Hosted elsewhere",
      installTarget: "example/herdr-remote",
      nextAction: "Bind a key.",
      windowsWsl: true,
    },
    {
      id: "herdr-plugin:example.sample",
      kind: "herdr-plugin",
      group: "Herdr plugins",
      label: "Sample Herdr plugin",
      description: "Sample Herdr capability",
      installTarget: "Yassimba/loom/plugins/herdr-sample",
      nextAction: "Run `herdr plugin list` to see the installed plugin.",
      windowsWsl: true,
    },
    {
      id: "tool:gh",
      kind: "tool",
      group: "Tools",
      label: "gh",
      description: "GitHub CLI",
      installTarget: "gh",
      nextAction: "Run `gh auth login` once.",
      bin: "gh",
      companions: [],
    },
  ]);
});

test("profile validation rejects ambiguous or stale presets", async (t) => {
  const cases = [
    {
      name: "duplicate ids",
      profiles: [
        { id: "same", label: "One", description: "One", resources: ["tool:gh"] },
        { id: "same", label: "Two", description: "Two", resources: ["tool:gh"] },
      ],
      error: /duplicate profile ids: same/,
    },
    {
      name: "duplicate labels",
      profiles: [
        { id: "one", label: "Same", description: "One", resources: ["tool:gh"] },
        { id: "two", label: "Same", description: "Two", resources: ["tool:gh"] },
      ],
      error: /duplicate profile labels: Same/,
    },
    {
      name: "empty resources",
      profiles: [{ id: "empty", label: "Empty", description: "Empty", resources: [] }],
      error: /every profile needs a non-empty/,
    },
    {
      name: "unknown resources",
      profiles: [{ id: "stale", label: "Stale", description: "Stale", resources: ["tool:gone"] }],
      error: /profile stale has unknown resources: tool:gone/,
    },
  ];

  for (const example of cases) {
    await t.test(example.name, async () => {
      const repoRoot = await createCatalogFixture();
      await writeFile(
        join(repoRoot, "manifest", "profiles.json"),
        JSON.stringify({ profiles: example.profiles }),
      );
      await assert.rejects(buildSetupCatalogDocument(repoRoot), example.error);
    });
  }
});

test("external Pi packages accept an exact Git commit source", async () => {
  const repoRoot = await createCatalogFixture();
  const source = `git:github.com/example/pi-example@${"a".repeat(40)}`;
  await writeFile(
    join(repoRoot, "manifest", "pi-packages.json"),
    JSON.stringify({
      packages: [
        {
          name: "pi-example",
          source,
          label: "example",
          description: "Example package",
          windowsSupport: "wsl",
        },
      ],
    }),
  );

  const example = (await buildSetupCatalog(repoRoot)).find(
    ({ id }) => id === "pi-package:pi-example",
  );

  assert.equal(example.source, source);
  assert.equal(example.version, undefined);
  assert.equal(example.windowsWsl, true);
});

test("every public Pi extension package is offered in the setup catalog", async () => {
  const catalog = await buildSetupCatalog(join(import.meta.dirname, ".."));
  const offeredPackages = new Set(
    catalog.filter(({ kind }) => kind === "pi-package").map(({ installTarget }) => installTarget),
  );
  const pluginsRoot = join(import.meta.dirname, "..", "plugins");
  const pluginEntries = await readdir(pluginsRoot, { withFileTypes: true });
  const missingPackages = [];

  for (const entry of pluginEntries) {
    if (!entry.isDirectory()) continue;
    const manifestPath = join(pluginsRoot, entry.name, "package.json");
    let manifest;
    try {
      manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }
    if (manifest.private || !manifest.pi?.extensions?.length) continue;
    if (!offeredPackages.has(manifest.name)) missingPackages.push(manifest.name);
  }

  assert.deepEqual(missingPackages, []);
});

test("tokei uses native assets where upstream publishes them", async () => {
  const catalog = await buildSetupCatalog(join(import.meta.dirname, ".."));
  const tokei = catalog.find(({ id }) => id === "tool:tokei");

  assert.equal(tokei.installTarget, "aqua:XAMPPRocky/tokei");
  assert.deepEqual(tokei.companions, ["cargo:tokei", "rust"]);
});

test("personal skills cannot enter the setup catalog", async () => {
  // personal/ lives outside skills/, so listing one in skills.sh.json fails
  // the lookup — the exclusion is structural, not a category check.
  const repoRoot = await createCatalogFixture();
  await writeFile(
    join(repoRoot, "skills.sh.json"),
    JSON.stringify({ groupings: [{ title: "Personal", skills: ["private"] }] }),
  );

  await assert.rejects(buildSetupCatalog(repoRoot), /reviewed skill not found: private/);
});

test("a provenance-only deps file is accepted", async () => {
  const repoRoot = await createCatalogFixture();
  await writeFile(
    join(repoRoot, "skills", "helper", "deps.yml"),
    `upstream:
  repository: https://github.com/example/skills
  path: skills/helper
  commit: ${"a".repeat(40)}
`,
  );

  const helper = (await buildSetupCatalog(repoRoot)).find(({ id }) => id === "skill:helper");

  assert.deepEqual(helper.dependencies, []);
});

test("incomplete upstream provenance is rejected", async () => {
  const repoRoot = await createCatalogFixture();
  await writeFile(
    join(repoRoot, "skills", "helper", "deps.yml"),
    "upstream:\n  repository: https://github.com/example/skills\n",
  );

  await assert.rejects(
    buildSetupCatalog(repoRoot),
    /upstream must contain repository, path, and commit/,
  );
});

test("a dependency cycle fails catalog generation", async () => {
  const repoRoot = await createCatalogFixture();
  await writeFile(join(repoRoot, "skills", "helper", "deps.yml"), "skills:\n  - reviewed\n");

  await assert.rejects(
    buildSetupCatalog(repoRoot),
    /skill dependency cycle: reviewed -> helper -> reviewed/,
  );
});

test("a skill cannot depend on an unreviewed skill", async () => {
  const repoRoot = await createCatalogFixture();
  await writeFile(join(repoRoot, "skills", "reviewed", "deps.yml"), "skills:\n  - unlisted\n");

  await assert.rejects(
    buildSetupCatalog(repoRoot),
    /skill reviewed depends on unreviewed skill: unlisted/,
  );
});

test("private packages cannot advertise themselves in setup", async () => {
  const repoRoot = await createCatalogFixture();
  const manifestPath = join(repoRoot, "plugins", "sample", "package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  await writeFile(manifestPath, JSON.stringify({ ...manifest, private: true }));

  await assert.rejects(buildSetupCatalog(repoRoot), /setup Pi package is private/);
});

test("bundledSkills explicitly links Pi packages to reviewed skills", async () => {
  const repoRoot = await createCatalogFixture();
  const path = join(repoRoot, "manifest", "pi-packages.json");
  const pkg = {
    name: "different-package-name",
    version: "1.0.0",
    label: "Package",
    description: "Bundled",
    bundledSkills: ["helper"],
  };
  await writeFile(path, JSON.stringify({ packages: [pkg] }));
  const catalog = await buildSetupCatalog(repoRoot);
  assert.deepEqual(catalog.find(({ installTarget }) => installTarget === pkg.name).bundledSkills, [
    "helper",
  ]);
  for (const invalid of [["private"], ["../helper"], ["helper", "helper"], "helper"]) {
    await writeFile(path, JSON.stringify({ packages: [{ ...pkg, bundledSkills: invalid }] }));
    await assert.rejects(buildSetupCatalog(repoRoot), /invalid bundledSkills/);
  }
});

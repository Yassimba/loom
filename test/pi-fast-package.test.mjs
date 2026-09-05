import assert from "node:assert/strict";
import { access, readdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packageDirectory = join(repoRoot, "plugins", "pi-fast");

async function directPiPackageImports() {
  const imports = new Set();
  const directory = packageDirectory;
  for (const file of await readdir(directory, { recursive: true })) {
    if (!file.endsWith(".ts")) continue;
    const source = await readFile(join(directory, file), "utf8");
    for (const match of source.matchAll(/from\s+["'](@earendil-works\/[^"']+)["']/g)) {
      imports.add(match[1]);
    }
  }
  return [...imports].sort();
}

test("the Fast Mode package is installable by Pi", async () => {
  const manifest = JSON.parse(await readFile(join(packageDirectory, "package.json"), "utf8"));

  assert.equal(manifest.name, "@yassimba/pi-fast");
  assert.equal(manifest.type, "module");
  assert.deepEqual(manifest.files, ["index.ts", "src", "README.md", "LICENSE"]);
  assert.deepEqual(manifest.pi.extensions, ["./index.ts"]);
  assert.deepEqual(Object.keys(manifest.peerDependencies).sort(), await directPiPackageImports());
  await access(join(packageDirectory, "index.ts"));
  await access(join(packageDirectory, "LICENSE"));
});

test("the Fast Mode package exposes only its focused public surface", async () => {
  const extensionModule = await import(pathToFileURL(join(packageDirectory, "index.ts")).href);

  assert.equal(typeof extensionModule.default, "function");
});

test("the Fast Mode fork records its upstream provenance", async () => {
  const readme = await readFile(join(packageDirectory, "README.md"), "utf8");
  assert.match(readme, /studioarray\/pi-openai-fast/);
  assert.match(readme, /e82ed32/);
});

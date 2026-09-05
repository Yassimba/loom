import { readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { buildSetupCatalogDocument } from "./catalog-lib.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDirectory, "..");
const outputPath = join(repoRoot, "cli", "loom", "setup-catalog.json");

async function generate({ check }) {
  const catalog = await buildSetupCatalogDocument(repoRoot);
  const content = `${JSON.stringify(catalog, null, 2)}\n`;
  if (check) {
    let current = "";
    try {
      current = await readFile(outputPath, "utf8");
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
    if (current !== content) {
      throw new Error("setup catalog is stale; run npm run catalog:generate and commit the result");
    }
    process.stdout.write("Setup catalog is current.\n");
    return;
  }

  await writeFile(outputPath, content);
  process.stdout.write(`Generated ${outputPath}.\n`);
}

await generate({ check: process.argv.includes("--check") });

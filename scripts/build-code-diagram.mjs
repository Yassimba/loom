import path from "node:path";
import { fileURLToPath } from "node:url";

import { build } from "esbuild";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outputFlag = process.argv.indexOf("--outdir");
const outdir =
  outputFlag === -1
    ? path.join(root, "skills/code-diagram/scripts")
    : path.resolve(process.argv[outputFlag + 1]);

await build({
  entryPoints: [path.join(root, "skills/code-diagram/scripts/src/viewer.tsx")],
  outdir,
  entryNames: "viewer",
  bundle: true,
  format: "iife",
  platform: "browser",
  target: ["chrome120", "firefox121", "safari17"],
  jsx: "automatic",
  define: { "process.env.NODE_ENV": '"production"' },
  legalComments: "inline",
  sourcemap: false,
  minify: true,
  logLevel: "info",
});

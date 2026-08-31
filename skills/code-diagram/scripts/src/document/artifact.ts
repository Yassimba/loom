import { build, type Plugin } from "esbuild";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

export interface ArtifactEvaluationOptions<T> {
  artifactPath: string;
  repo: string;
  errorCode: string;
  moduleAliases?: Readonly<Record<string, string>>;
  read(module: Record<string, unknown>): T | Promise<T>;
}

/** Bundle and evaluate one trusted authoring artifact, always cleaning up its temporary module. */
export async function evaluateArtifact<T>({
  artifactPath,
  repo,
  errorCode,
  moduleAliases = {},
  read,
}: ArtifactEvaluationOptions<T>): Promise<T> {
  try {
    const output = await build({
      absWorkingDir: repo,
      bundle: true,
      platform: "node",
      format: "esm",
      packages: "external",
      target: ["node22"],
      write: false,
      entryPoints: [artifactPath],
      plugins: [aliasPlugin(moduleAliases)],
      logLevel: "silent",
    });
    const code = output.outputFiles[0]?.text;
    if (!code) throw new Error("compiler produced no output");

    const temp = await mkdtemp(path.join(tmpdir(), "code-diagram-artifact-"));
    const modulePath = path.join(temp, "artifact.mjs");
    try {
      await writeFile(modulePath, code);
      const loaded = (await import(pathToFileURL(modulePath).href)) as Record<string, unknown>;
      return await read(loaded);
    } finally {
      await rm(temp, { recursive: true, force: true });
    }
  } catch (error) {
    throw new Error(`${errorCode}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

function aliasPlugin(aliases: Readonly<Record<string, string>>): Plugin {
  return {
    name: "code-diagram-artifact-aliases",
    setup(pluginBuild) {
      for (const [specifier, target] of Object.entries(aliases))
        pluginBuild.onResolve({ filter: exactPattern(specifier) }, () => ({ path: target }));
    },
  };
}

function exactPattern(value: string): RegExp {
  return new RegExp(`^${value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`);
}

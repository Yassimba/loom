import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

// Pi's exec timeout only sends SIGTERM; a wrapper can ignore it indefinitely.
export const execVersionCommand: ExtensionAPI["exec"] = async (command, args, options) => {
  const { stdout, stderr } = await promisify(execFile)(command, args, {
    signal: options?.signal,
    timeout: options?.timeout,
    killSignal: "SIGKILL",
    maxBuffer: 4096,
    encoding: "utf8",
  });
  return { stdout, stderr, code: 0, killed: false };
};

const MANIFEST_URL = "https://raw.githubusercontent.com/Yassimba/loom/main/manifest/loom.toml";
const VERSION = String.raw`(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)`;

export function availableLoomVersion(installed: string, manifest: string): string | undefined {
  const current = installed.trim().match(new RegExp(`^loom (${VERSION})$`));
  // The published manifest requires one tool per line (also used by the installers).
  // Match only the Loom executable in [tools], never another component's release.
  const tools = manifest.split(/^\[tools\]\s*$/m)[1]?.split(/^\[/m)[0];
  const latest = tools?.match(
    new RegExp(
      String.raw`^"github:Yassimba/loom\[exe=loom\]"\s*=\s*\{\s*version\s*=\s*"loom-v(${VERSION})"[^\r\n]*\}\s*(?:#.*)?$`,
      "m",
    ),
  );
  if (!current || !latest) return undefined;
  for (let index = 2; index <= 4; index++) {
    const difference = BigInt(latest[index]) - BigInt(current[index]);
    if (difference !== 0n) return difference > 0n ? latest[1] : undefined;
  }
  return undefined;
}

export async function checkLoomUpdate(
  exec: ExtensionAPI["exec"],
  shutdown: AbortSignal,
): Promise<string | undefined> {
  if (/^(1|true|yes)$/i.test(process.env.PI_OFFLINE?.trim() ?? "")) return undefined;
  const signal = AbortSignal.any([shutdown, AbortSignal.timeout(4000)]);
  try {
    const result = await exec("loom", ["--version"], { timeout: 1500, signal });
    if (result.code !== 0 || result.killed || signal.aborted) return undefined;
    if (!new RegExp(`^loom ${VERSION}$`).test(result.stdout.trim())) return undefined;
    const response = await fetch(MANIFEST_URL, { signal });
    if (!response.ok) return undefined;
    return availableLoomVersion(result.stdout, await response.text());
  } catch {
    // Missing Loom, offline hosts, and failed checks must not interrupt startup.
    return undefined;
  }
}

/**
 * Resolve Pi documentation paths dynamically from the running Pi runtime.
 *
 * Pi bakes its docs/examples paths into the system prompt text at launch time,
 * but those paths change between Pi versions (e.g. Nix store hashes), so we
 * resolve them directly from Pi's package asset path helpers instead of
 * scraping the system prompt or persisting concrete paths in config.
 */
import { tmpdir } from "node:os";
import * as piCodingAgent from "@earendil-works/pi-coding-agent";
import type { AllowedPath } from "../../src/core/paths";

/**
 * Resolve Pi documentation paths dynamically from the running Pi runtime.
 *
 * Pi bakes its docs/examples paths into the system prompt text at launch time,
 * but those paths change between Pi versions (e.g. Nix store hashes), so we
 * resolve them directly from Pi's package asset path helpers instead of
 * scraping the system prompt or persisting concrete paths in config.
 *
 * These helpers honor `PI_PACKAGE_DIR` (Nix/Guix) and walk to the package
 * root for npm global installs and Bun binaries, so they always match the
 * Pi version actually running. They depend only on the process environment
 * and are fixed for the process lifetime — resolve once, no per-turn work.
 *
 * The README is a single file grant; docs and examples are directory grants.
 *
 * If the helpers are not exported by the runtime (e.g. Oh My Pi), this
 * returns an empty array so the extension can still load and only the
 * documentation grants are skipped.
 */
export function temporaryPaths(): AllowedPath[] {
  const paths = new Set(["/tmp", tmpdir()]);
  if (process.platform === "darwin") {
    paths.add("/private/tmp");
    if (tmpdir().startsWith("/var/")) paths.add(`/private${tmpdir()}`);
  }
  return [...paths].map((path) => ({ kind: "directory", path }));
}

export function piDocumentationPaths(): AllowedPath[] {
  const { getReadmePath, getDocsPath, getExamplesPath } = piCodingAgent;

  if (
    typeof getReadmePath !== "function" ||
    typeof getDocsPath !== "function" ||
    typeof getExamplesPath !== "function"
  ) {
    return [];
  }

  return [
    { kind: "file", path: getReadmePath() },
    { kind: "directory", path: getDocsPath() },
    { kind: "directory", path: getExamplesPath() },
  ];
}

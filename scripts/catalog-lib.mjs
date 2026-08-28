import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import { parse } from "yaml";

function parseFrontmatter(content, path) {
  const match = content.match(/^---\s*\n([\s\S]*?)\n---(?:\s*\n|$)/);
  if (!match) throw new Error(`missing frontmatter: ${path}`);
  const value = parse(match[1]);
  if (!value || typeof value !== "object") throw new Error(`invalid frontmatter: ${path}`);
  return value;
}

function readSkillEntries(manifest) {
  if (!Array.isArray(manifest?.groupings)) {
    throw new Error("skills.sh.json must contain a groupings array");
  }
  const entries = manifest.groupings.flatMap((group) =>
    Array.isArray(group?.skills) ? group.skills.map((name) => ({ name, group: group.title })) : [],
  );
  if (
    !entries.every(
      ({ name, group }) =>
        typeof name === "string" &&
        name.length > 0 &&
        typeof group === "string" &&
        group.length > 0,
    )
  ) {
    throw new Error("every reviewed skill and grouping title must be a non-empty string");
  }
  const names = entries.map(({ name }) => name);
  const duplicates = names.filter((name, index) => names.indexOf(name) !== index);
  if (duplicates.length > 0) {
    throw new Error(`duplicate reviewed skill names: ${[...new Set(duplicates)].join(", ")}`);
  }
  return entries;
}

async function indexSkillDirectories(repoRoot) {
  const index = new Map();
  const skillsRoot = join(repoRoot, "skills");
  for (const skillEntry of await readdir(skillsRoot, { withFileTypes: true })) {
    if (!skillEntry.isDirectory()) continue;
    const skillPath = join(skillsRoot, skillEntry.name, "SKILL.md");
    try {
      const frontmatter = parseFrontmatter(await readFile(skillPath, "utf8"), skillPath);
      const dependencies = await readSkillDependencies(join(skillsRoot, skillEntry.name));
      index.set(skillEntry.name, { frontmatter, path: skillPath, dependencies });
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  return index;
}

// skills/<name>/deps.yml — the skills this skill invokes; installers pull
// them in transitively. Validated against the reviewed set by the caller.
async function readSkillDependencies(skillRoot) {
  const depsPath = join(skillRoot, "deps.yml");
  let raw;
  try {
    raw = await readFile(depsPath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") return { skills: [], tools: [] };
    throw error;
  }
  const value = parse(raw);
  const skills = value?.skills ?? [];
  const tools = value?.tools ?? [];
  const names = (list) =>
    Array.isArray(list) && list.every((name) => typeof name === "string" && name.length > 0);
  if (!names(skills) || !names(tools) || skills.length + tools.length === 0) {
    throw new Error(`deps.yml must hold non-empty skills and/or tools lists of names: ${depsPath}`);
  }
  return { skills, tools };
}

export async function readReviewedSkillCatalog(repoRoot) {
  const manifest = JSON.parse(await readFile(join(repoRoot, "skills.sh.json"), "utf8"));
  const entries = readSkillEntries(manifest);
  const index = await indexSkillDirectories(repoRoot);

  const reviewed = new Set(entries.map(({ name }) => name));
  rejectDependencyCycles(entries, index);
  return entries.map(({ name, group }) => {
    const candidate = index.get(name);
    if (!candidate) throw new Error(`reviewed skill not found: ${name}`);
    if (candidate.frontmatter.name !== name) {
      throw new Error(`skill directory and frontmatter name differ: ${name}`);
    }
    if (typeof candidate.frontmatter.description !== "string") {
      throw new Error(`skill description is missing: ${name}`);
    }
    for (const dependency of candidate.dependencies.skills) {
      if (!reviewed.has(dependency)) {
        throw new Error(`skill ${name} depends on unreviewed skill: ${dependency}`);
      }
    }
    return {
      id: `skill:${name}`,
      kind: "skill",
      group,
      label: name,
      description: candidate.frontmatter.description,
      installTarget: name,
      nextAction: `Ask your coding agent to use the ${name} skill.`,
      dependencies: candidate.dependencies.skills,
      toolDependencies: candidate.dependencies.tools,
    };
  });
}

// The skill dep graph must stay acyclic (AGENTS.md); enforcing it here puts
// the check on the `npm run check` path instead of only the manually-run
// `sync-skills.sh deps`. DFS with a path stack names the cycle it finds.
function rejectDependencyCycles(entries, index) {
  const visited = new Set();
  const stack = [];
  const visit = (name) => {
    const position = stack.indexOf(name);
    if (position !== -1) {
      throw new Error(`skill dependency cycle: ${[...stack.slice(position), name].join(" -> ")}`);
    }
    if (visited.has(name)) return;
    visited.add(name);
    stack.push(name);
    for (const dependency of index.get(name)?.dependencies?.skills ?? []) visit(dependency);
    stack.pop();
  };
  for (const { name } of entries) visit(name);
}

export async function readPiPackageCatalog(repoRoot) {
  const packagesRoot = join(repoRoot, "plugins");
  const resources = await readExternalPiPackages(repoRoot);
  for (const entry of await readdir(packagesRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const manifestPath = join(packagesRoot, entry.name, "package.json");
    let raw;
    try {
      raw = await readFile(manifestPath, "utf8");
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }
    const manifest = JSON.parse(raw);
    const catalog = manifest?.loom?.catalog;
    if (!catalog) continue;
    if (manifest.private) throw new Error(`setup Pi package is private: ${manifestPath}`);
    if (
      typeof catalog.label !== "string" ||
      typeof catalog.description !== "string" ||
      typeof manifest.name !== "string"
    ) {
      throw new Error(`invalid loom.catalog metadata: ${manifestPath}`);
    }
    resources.push({
      id: `pi-package:${manifest.name}`,
      kind: "pi-package",
      group: "Pi packages",
      label: catalog.label,
      description: catalog.description,
      installTarget: manifest.name,
      nextAction: catalog.nextAction ?? "Start Pi and use the installed package.",
    });
  }
  const names = resources.map((resource) => resource.installTarget);
  const duplicates = names.filter((name, index) => names.indexOf(name) !== index);
  if (duplicates.length > 0) {
    throw new Error(`duplicate Pi package names: ${[...new Set(duplicates)].join(", ")}`);
  }
  return resources.sort((left, right) => left.label.localeCompare(right.label));
}

/// Upstream Pi packages from manifest/pi-packages.json — offered next to the
/// repo's own workspaces, installed verbatim from npm by name.
async function readExternalPiPackages(repoRoot) {
  let raw;
  try {
    raw = await readFile(join(repoRoot, "manifest", "pi-packages.json"), "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }
  const meta = JSON.parse(raw);
  return meta.packages.map(({ name, version, label, description, nextAction }) => {
    if (!name || !label || !description) {
      throw new Error(`pi-packages.json entries need name, label, and description`);
    }
    if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
      throw new Error(`pi-packages.json: ${name} needs an exact version pin, got ${version}`);
    }
    return {
      id: `pi-package:${name}`,
      kind: "pi-package",
      group: "Pi packages",
      label,
      description,
      installTarget: name,
      version,
      nextAction: nextAction ?? "Start Pi and use the installed package.",
    };
  });
}

function parseTomlString(content, key, path) {
  const header = content.split(/^\s*\[/m, 1)[0];
  const match = header.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, "m"));
  if (!match) throw new Error(`missing ${key} in ${path}`);
  return match[1];
}

export async function readHerdrPluginCatalog(repoRoot) {
  const pluginsRoot = join(repoRoot, "plugins");
  const resources = [];
  for (const entry of await readdir(pluginsRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const manifestPath = join(pluginsRoot, entry.name, "herdr-plugin.toml");
    let content;
    try {
      content = await readFile(manifestPath, "utf8");
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }
    const id = parseTomlString(content, "id", manifestPath);
    resources.push({
      id: `herdr-plugin:${id}`,
      kind: "herdr-plugin",
      group: "Herdr plugins",
      label: parseTomlString(content, "name", manifestPath),
      description: parseTomlString(content, "description", manifestPath),
      installTarget: `Yassimba/loom/plugins/${entry.name}`,
      nextAction: "Run `herdr plugin list` to see the installed plugin.",
    });
  }
  const external = JSON.parse(
    await readFile(join(repoRoot, "manifest", "herdr-plugins.json"), "utf8"),
  );
  for (const plugin of external.plugins) {
    resources.push({
      id: `herdr-plugin:${plugin.id}`,
      kind: "herdr-plugin",
      group: "Herdr plugins",
      label: plugin.label,
      description: plugin.description,
      installTarget: plugin.installTarget,
      nextAction: plugin.nextAction,
    });
  }
  return resources.sort((left, right) => left.label.localeCompare(right.label));
}

/// Optional tools from the published mise manifest: the manifest carries the
/// pins (the menu), manifest/tools.json carries the wizard metadata. The two
/// key sets must match exactly — a pin without metadata can't be offered, and
/// metadata without a pin can't be installed.
export async function readToolCatalog(repoRoot) {
  const manifestRaw = await readFile(join(repoRoot, "manifest", "loom.toml"), "utf8");
  const meta = JSON.parse(await readFile(join(repoRoot, "manifest", "tools.json"), "utf8"));

  const withoutCore = manifestRaw.replace(/^# core:begin[\s\S]*?^# core:end$/m, "");
  const toolsSection = withoutCore.slice(withoutCore.indexOf("[tools]"));
  const optionalKeys = [...toolsSection.matchAll(/^(?:"([^"]+)"|([A-Za-z0-9@._/-]+))\s*=/gm)]
    .map((match) => match[1] ?? match[2])
    .filter((key) => key !== "tools");

  const claimed = new Set(meta.tools.flatMap((tool) => [tool.key, ...(tool.companions ?? [])]));
  const missingMeta = optionalKeys.filter((key) => !claimed.has(key));
  const missingPin = meta.tools
    .flatMap((tool) => [tool.key, ...(tool.companions ?? [])])
    .filter((key) => !optionalKeys.includes(key))
    .map((key) => ({ key }));
  if (missingMeta.length > 0) {
    throw new Error(`manifest tools missing metadata in tools.json: ${missingMeta.join(", ")}`);
  }
  if (missingPin.length > 0) {
    throw new Error(
      `tools.json entries missing from the manifest: ${missingPin.map((tool) => tool.key).join(", ")}`,
    );
  }

  return meta.tools.map((tool) => ({
    id: `tool:${tool.label}`,
    kind: "tool",
    group: "Tools",
    label: tool.label,
    description: tool.description,
    installTarget: tool.key,
    nextAction: tool.nextAction,
    // The executable a PATH probe should look for; most tools are named
    // after themselves (mermaid-cli ships mmdc, marp-cli ships marp).
    bin: tool.bin ?? tool.label,
    // Per-OS variant keys installed alongside the primary; mise os filters
    // decide which actually applies on each machine.
    companions: tool.companions ?? [],
  }));
}

export async function buildSetupCatalog(repoRoot) {
  const [piPackages, skills, herdrPlugins, tools] = await Promise.all([
    readPiPackageCatalog(repoRoot),
    readReviewedSkillCatalog(repoRoot),
    readHerdrPluginCatalog(repoRoot),
    readToolCatalog(repoRoot),
  ]);
  // Tool deps are declared by wizard label; resolve to install targets so
  // the CLI's one expansion walk covers skills and tools alike.
  const toolTargets = new Map(tools.map((tool) => [tool.label, tool.installTarget]));
  for (const skill of skills) {
    const targets = (skill.toolDependencies ?? []).map((label) => {
      const target = toolTargets.get(label);
      if (!target) {
        throw new Error(`skill ${skill.label} depends on unknown tool: ${label}`);
      }
      return target;
    });
    skill.dependencies = [...skill.dependencies, ...targets];
    delete skill.toolDependencies;
  }
  return [...piPackages, ...skills, ...herdrPlugins, ...tools];
}

/// Wizard selection bundles from manifest/presets.json; every target must
/// resolve to a catalog resource so a preset can never silently rot.
export async function readSetupPresets(repoRoot, resources) {
  const meta = JSON.parse(await readFile(join(repoRoot, "manifest", "presets.json"), "utf8"));
  const targets = new Set(resources.map((resource) => resource.installTarget));
  for (const preset of meta.presets) {
    const unknown = preset.targets.filter((target) => !targets.has(target));
    if (unknown.length > 0) {
      throw new Error(`preset ${preset.id} names unknown install targets: ${unknown.join(", ")}`);
    }
  }
  return meta.presets.map(({ id, label, description, targets }) => ({
    id,
    label,
    description,
    targets,
  }));
}

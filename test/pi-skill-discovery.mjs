// Exercise Pi's actual package manager and collision detector, never install packages.
import { DefaultPackageManager } from "../node_modules/@earendil-works/pi-coding-agent/dist/core/package-manager.js";
import { SettingsManager } from "../node_modules/@earendil-works/pi-coding-agent/dist/core/settings-manager.js";
import { loadSkills } from "../node_modules/@earendil-works/pi-coding-agent/dist/core/skills.js";

const [cwd, agentDir] = process.argv.slice(2);
const settingsManager = SettingsManager.create(cwd, agentDir, { projectTrusted: true });
const manager = new DefaultPackageManager({ cwd, agentDir, settingsManager });
const resources = await manager.resolve(async () => "error");
const result = loadSkills({
  cwd,
  agentDir,
  includeDefaults: false,
  skillPaths: resources.skills
    .filter((resource) => resource.enabled)
    .map((resource) => resource.path),
});
console.log(JSON.stringify(result));

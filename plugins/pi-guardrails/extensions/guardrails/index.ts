import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { configLoader } from "../../src/shared/config";
import { registerPathAccess } from "../path-access";
import { registerAddDir } from "./commands/add-dir";
import { registerGuardrailsExamplesCommand } from "./commands/examples";
import { registerGuardrailsOnboardingCommand } from "./commands/onboarding";
import { isOnboardingPending } from "./commands/onboarding/config";
import { registerGuardrailsSettings } from "./commands/settings";

export default async function guardrails(pi: ExtensionAPI) {
  await configLoader.load();
  const workspace = registerAddDir(pi);
  registerPathAccess(pi, workspace);

  registerGuardrailsSettings(pi);

  registerGuardrailsExamplesCommand(pi);
  if (isOnboardingPending(configLoader.getRawConfig("global"))) {
    registerGuardrailsOnboardingCommand(pi);
  }
  pi.on("session_start", (_event, ctx) => {
    const warnings = configLoader.drainMessages();
    if (warnings.length === 1) {
      ctx.ui.notify(warnings[0], "warning");
    } else if (warnings.length > 1) {
      ctx.ui.notify(
        ["Guardrails warnings:", ...warnings.map((warning) => `- ${warning}`)].join("\n"),
        "warning",
      );
    }
  });
}

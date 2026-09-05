import type {
  BeforeProviderRequestEvent,
  ExtensionAPI,
  ExtensionCommandContext,
  ExtensionContext,
} from "@earendil-works/pi-coding-agent";
import { defaultFastConfig, type FastConfig, loadConfig, saveDesiredActive } from "./src/config.ts";
import {
  FastFooter,
  type FooterContext,
  type FooterModel,
  type FooterTheme,
} from "./src/footer.ts";

export const FAST_COMMAND = "fast";
export const FAST_FLAG = "fast";
export const FAST_STATUS_KEY = "pi-fast";
export const FAST_DESIRED_HANDOFF_ENV = "PI_FAST_DESIRED";
export const PRIORITY_SERVICE_TIER = "priority";

export const FAST_REQUESTED_INACTIVE_NO_MODEL_WARNING = "Select a model to use Fast Mode.";
export const FAST_REQUESTED_INACTIVE_UNSUPPORTED_MODEL_WARNING =
  "Fast Mode is paused because this model isn't in supportedModels. Switch models or edit pi-fast.json.";

/** Shares the /fast choice with subagents started after the toggle. */
function writeFastDesiredHandoff(desiredActive: boolean): void {
  process.env[FAST_DESIRED_HANDOFF_ENV] = desiredActive ? "1" : "0";
}

function readFastDesiredHandoff(): { desiredActive?: boolean; warning?: string } {
  const value = process.env[FAST_DESIRED_HANDOFF_ENV];
  if (value === undefined) return {};
  if (value === "1" || value === "0") return { desiredActive: value === "1" };
  return {
    warning: `Ignoring ${FAST_DESIRED_HANDOFF_ENV}=${JSON.stringify(value)}. Set it to 1 for on or 0 for off.`,
  };
}

export default function piFast(pi: ExtensionAPI): void {
  let config: FastConfig = defaultFastConfig();
  let configLoad: Promise<{ warnings: string[] }> | undefined;
  let configLoaded = false;
  let desiredActive = false;
  let currentModel: FooterModel | undefined;
  let installedFooter: FastFooter | undefined;
  let ownsStatus = false;
  let footerView: FooterContext | undefined;

  pi.registerFlag(FAST_FLAG, {
    description: "Request priority responses for this session",
    type: "boolean",
    default: false,
  });

  function isActive(): boolean {
    if (!desiredActive || !currentModel?.provider || !currentModel.id) return false;
    const { provider, id } = currentModel;
    return (
      config.supportedModels.includes(`${provider}/${id}`) ||
      config.supportedModels.includes(`${provider}/*`)
    );
  }

  function inactiveReason(): "no-model" | "unsupported-model" | undefined {
    if (!desiredActive || isActive()) return undefined;
    return currentModel?.provider && currentModel.id ? "unsupported-model" : "no-model";
  }

  function notify(
    ui: ExtensionContext["ui"] | undefined,
    message: string,
    type: "info" | "warning" | "error",
  ): void {
    try {
      ui?.notify?.(message, type);
    } catch {
      // A broken UI notification must not stop the session or /fast command.
    }
  }

  function deliverWarnings(
    warnings: readonly string[],
    ui: ExtensionContext["ui"] | undefined,
  ): void {
    for (const message of warnings) {
      if (typeof ui?.notify === "function") notify(ui, message, "warning");
      else console.warn(`[pi-fast] ${message}`);
    }
  }

  /** Warns once when a model change or toggle leaves Fast Mode unable to run. */
  function transition(
    update: { desiredActive?: boolean; model?: FooterModel | undefined },
    ui: ExtensionContext["ui"] | undefined,
  ): void {
    const wasRequestedInactive = inactiveReason() !== undefined;
    if (update.desiredActive !== undefined) desiredActive = update.desiredActive;
    if (Object.hasOwn(update, "model")) currentModel = update.model;
    const reason = inactiveReason();
    if (reason !== undefined && !wasRequestedInactive) {
      notify(
        ui,
        reason === "no-model"
          ? FAST_REQUESTED_INACTIVE_NO_MODEL_WARNING
          : FAST_REQUESTED_INACTIVE_UNSUPPORTED_MODEL_WARNING,
        "warning",
      );
    }
  }

  function syncFooter(ctx: ExtensionContext, model = ctx.model as FooterModel | undefined): void {
    footerView = {
      model,
      sessionManager: ctx.sessionManager,
      modelRegistry: ctx.modelRegistry,
      getContextUsage: () => ctx.getContextUsage(),
    };
    const ui = ctx.ui;
    const showStatus = config.footer.mode === "status" && isActive();
    if (typeof ui?.setStatus === "function") {
      ui.setStatus(FAST_STATUS_KEY, showStatus ? "fast" : undefined);
      ownsStatus = showStatus;
    }

    if (config.footer.mode !== "replace") {
      clearFooter(ctx);
      return;
    }
    if (installedFooter?.isOwnedByExtension()) {
      installedFooter.invalidate();
      return;
    }
    installedFooter = undefined;
    if (typeof ui?.setFooter !== "function") return;
    ui.setFooter((tui, theme, footerData) => {
      const footer = new FastFooter({
        getContext: () => footerView,
        footerData,
        theme: theme as FooterTheme,
        isFastActive: isActive,
        getThinkingLevel: () => pi.getThinkingLevel(),
        fastLabelColors: {
          dark: config.footer.darkFastColor,
          light: config.footer.lightFastColor,
          vars: config.footer.vars,
        },
        tui,
      });
      installedFooter = footer;
      return footer;
    });
  }

  function clearFooter(ctx: ExtensionContext | undefined): void {
    if (!installedFooter) return;
    if (installedFooter.isOwnedByExtension()) {
      installedFooter.dispose();
      if (typeof ctx?.ui?.setFooter === "function") ctx.ui.setFooter(undefined);
    }
    installedFooter = undefined;
  }

  async function loadConfigOnce(cwd: string): Promise<string[]> {
    if (configLoaded) return [];
    configLoad ??= loadConfig(cwd).then((result) => {
      config = result.config;
      configLoaded = true;
      return result;
    });
    return (await configLoad).warnings;
  }

  /** Loads settings once. The flag overrides the environment, which overrides the saved choice. */
  async function startSession(ctx: ExtensionContext, model = ctx.model as FooterModel | undefined) {
    const warnings = [...(await loadConfigOnce(ctx.cwd))];
    const handoff = readFastDesiredHandoff();
    if (handoff.warning !== undefined) warnings.push(handoff.warning);
    deliverWarnings(warnings, ctx.ui);
    const startupFastOverride = pi.getFlag(FAST_FLAG) === true;
    if (startupFastOverride) writeFastDesiredHandoff(true);
    const desired = startupFastOverride
      ? true
      : (handoff.desiredActive ?? (config.persistState ? config.desiredActive : false));
    transition({ desiredActive: desired, model }, ctx.ui);
    syncFooter(ctx, model);
  }

  pi.registerCommand(FAST_COMMAND, {
    description: "Turn priority requests on or off",
    handler: async (args: string, ctx: ExtensionCommandContext) => {
      if (args.trim().length > 0) {
        notify(ctx.ui, "Run /fast without arguments to turn Fast Mode on or off.", "error");
        return;
      }
      if (!configLoaded) await startSession(ctx);
      transition(
        { desiredActive: !desiredActive, model: ctx.model as FooterModel | undefined },
        ctx.ui,
      );
      writeFastDesiredHandoff(desiredActive);
      config = { ...config, desiredActive };
      if (config.persistState) {
        const saved = await saveDesiredActive(ctx.cwd, desiredActive);
        deliverWarnings(saved.warnings, ctx.ui);
      }
      syncFooter(ctx);
    },
  });

  pi.on("session_start", async (_event, ctx) => {
    await startSession(ctx);
  });

  pi.on("session_shutdown", async (_event, ctx: ExtensionContext) => {
    clearFooter(ctx);
    if (ownsStatus && typeof ctx.ui?.setStatus === "function") {
      ctx.ui.setStatus(FAST_STATUS_KEY, undefined);
    }
    ownsStatus = false;
  });

  pi.on("model_select", async (event, ctx: ExtensionContext) => {
    const model = event.model as FooterModel | undefined;
    if (!configLoaded) {
      await startSession(ctx, model);
      return;
    }
    transition({ model }, ctx.ui);
    syncFooter(ctx, model);
  });

  pi.on("thinking_level_select", (_event, ctx: ExtensionContext) => {
    if (configLoaded) syncFooter(ctx);
  });

  pi.on("before_provider_request", (event: BeforeProviderRequestEvent) => {
    const payload = event.payload;
    if (typeof payload !== "object" || payload === null || Array.isArray(payload)) {
      return undefined;
    }
    const prototype = Object.getPrototypeOf(payload);
    if ((prototype !== Object.prototype && prototype !== null) || !isActive()) {
      return undefined;
    }
    return { ...(payload as Record<string, unknown>), service_tier: PRIORITY_SERVICE_TIER };
  });
}

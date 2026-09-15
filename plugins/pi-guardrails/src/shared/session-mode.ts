import type { ResolvedConfig } from "./config";

export type GuardrailsSessionMode = "ask" | "free" | "yolo";
export const GUARDRAILS_MODE_CHANGED_EVENT = "guardrails:mode:changed";

export interface GuardrailsModeChangedPayload {
  mode: GuardrailsSessionMode;
}

const MODES: GuardrailsSessionMode[] = ["ask", "free", "yolo"];

let mode: GuardrailsSessionMode = "ask";
let yoloConfirmed = false;

export function configuredSessionMode(config: ResolvedConfig): GuardrailsSessionMode {
  if (!config.enabled) return "yolo";
  if (!config.features.pathAccess || config.pathAccess.mode === "allow") return "free";
  return "ask";
}

export function getSessionMode(): GuardrailsSessionMode {
  return mode;
}

export function resetSessionMode(next: GuardrailsSessionMode): void {
  mode = next;
  yoloConfirmed = next === "yolo";
}

export async function cycleSessionMode(
  confirmYolo: () => Promise<boolean>,
): Promise<GuardrailsSessionMode> {
  const next = MODES[(MODES.indexOf(mode) + 1) % MODES.length];
  if (next === "yolo" && !yoloConfirmed) {
    if (!(await confirmYolo())) return mode;
    yoloConfirmed = true;
  }
  mode = next;
  return mode;
}

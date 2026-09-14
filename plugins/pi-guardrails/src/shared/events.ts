import { randomUUID } from "node:crypto";
import type { Action } from "../core/types";

export const GUARDRAILS_PROMPT_OPENED_EVENT = "guardrails:prompt:opened";
export const GUARDRAILS_PROMPT_CLOSED_EVENT = "guardrails:prompt:closed";
export type GuardrailsFeatureId = "policies" | "permissionGate" | "pathAccess";

export interface GuardrailsEventBase {
  source: "guardrails";
  feature: GuardrailsFeatureId;
  timestamp: string;
}

export interface GuardrailsPrompt<TMeta = unknown> {
  /** What kind of prompt was shown */
  kind: "confirmation" | "permission";
  /** The feature-specific metadata about the risk */
  metadata?: TMeta;
}

export interface GuardrailsPromptWithId<TMeta = unknown> extends GuardrailsPrompt<TMeta> {
  /** Correlates this event with its matching prompt-closed event. */
  id: string;
}

export interface GuardrailsPromptEventDetails<
  TMeta = unknown,
  TPrompt extends GuardrailsPrompt<TMeta> = GuardrailsPrompt<TMeta>,
> {
  feature: GuardrailsFeatureId;
  action: Action;
  reason: string;
  prompt: TPrompt;
  context?: {
    toolName?: string;
    input?: Record<string, unknown>;
  };
}

export type GuardrailsPromptOpenedPayload<TMeta = unknown> = GuardrailsEventBase &
  GuardrailsPromptEventDetails<TMeta, GuardrailsPromptWithId<TMeta>>;

export type GuardrailsPromptClosedPayload = GuardrailsEventBase & {
  prompt: {
    /** The ID from the matching prompt-opened event. */
    id: string;
  };
};

function timestamp(): string {
  return new Date().toISOString();
}

export function createPromptOpenedPayload<TMeta = unknown>(
  event: GuardrailsPromptEventDetails<TMeta>,
): GuardrailsPromptOpenedPayload<TMeta> {
  return {
    source: "guardrails",
    timestamp: timestamp(),
    ...event,
    prompt: {
      ...event.prompt,
      id: randomUUID(),
    },
  };
}

export function createPromptClosedPayload(
  opened: Pick<GuardrailsPromptOpenedPayload, "feature" | "prompt">,
): GuardrailsPromptClosedPayload {
  return {
    source: "guardrails",
    feature: opened.feature,
    timestamp: timestamp(),
    prompt: { id: opened.prompt.id },
  };
}

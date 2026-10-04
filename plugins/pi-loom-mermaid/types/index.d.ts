/** The Mermaid fence a streaming reply has open, as written so far; null between fences. */
export type ArrivingFence = string | null;

declare module "claude-code" {
  interface PluginState {
    "loom-mermaid": { arriving: ArrivingFence };
  }
}

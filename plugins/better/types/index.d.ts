export type Run = { agentId: string; tools: number; last: string };

declare module "claude-code" {
  interface PluginState {
    better: { run: Run | null };
  }
}

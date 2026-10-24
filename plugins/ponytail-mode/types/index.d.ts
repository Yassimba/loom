/** How hard the ponytail skill pushes for less code; `off` sends no ponytail section. */
export type Level = "lite" | "full" | "ultra" | "off";

declare module "claude-code" {
  interface PluginState {
    "ponytail-mode": { level: Level };
  }
}

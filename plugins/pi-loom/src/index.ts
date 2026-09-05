import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { truncateToWidth } from "@earendil-works/pi-tui";
import { checkLoomUpdate, execVersionCommand } from "./update.ts";

// Frozen 24-column braille rendering of assets/loom-logo.svg; no runtime rasterizer.
const LOGO = [
  "          ⢠⣴⣦⡀",
  "        ⢀⣴⣿⣿⣿⣿⣦⡀",
  "       ⢶⣿⣿⣿⠋⠙⣿⣿⣿⡦",
  "    ⢀⣤⡀ ⠙⢿⡿⣢⣾⣿⡿⠋ ⢀⣤⡀",
  "  ⢀⣴⣿⣿⣿⣦⡀⢠⣾⣿⡿⢋⣄⢀⣴⣿⣿⣿⣦⡀",
  "⢀⣴⣿⣿⡿⠛⢿⣿⣿⣎⠻⠋⠐⢿⣿⣷⣝⠟⠙⢿⣿⣿⣦⡀",
  "⠙⢿⣿⣿⣷⣄⣴⣝⢿⣿⣷⠄⣠⣦⡙⢿⣿⣷⣤⣾⣿⣿⡿⠋",
  "  ⠙⢿⣿⣿⡿⠋ ⠉⣡⣾⣿⡿⠁ ⠙⢿⣿⣿⡿⠋",
  "    ⠙⠋  ⣠⣾⣿⡿⢫⣾⣷⣄  ⠙⠋",
  "       ⠺⣿⣿⣿⣄⣠⣿⣿⣿⠗",
  "        ⠈⠻⣿⣿⣿⣿⠟⠁",
  "          ⠘⢿⠟⠁",
];

export default function (pi: ExtensionAPI, exec = execVersionCommand) {
  let checked = false;
  const shutdown = new AbortController();

  pi.on("session_start", (event, ctx) => {
    if (ctx.mode !== "tui" || !ctx.hasUI) return;
    ctx.ui.setHeader((_tui, theme) => ({
      render(width) {
        if (width < 48) return [truncateToWidth(theme.bold("Loom"), width, "")];
        const labels = [
          theme.bold("Loom"),
          theme.fg("muted", "Your opinionated agent setup."),
          "",
          `${theme.bold("/loom")} ${theme.fg("dim", "Set up, add tools, or update.")}`,
        ];
        return LOGO.map((line, index) => {
          const label = labels[index] ?? "";
          // Match the SVG's #8038E9 independently of the active Pi theme.
          return truncateToWidth(
            `\x1b[38;2;128;56;233m${line.padEnd(28)}\x1b[39m${label}`,
            width,
            "",
          );
        });
      },
      invalidate() {},
    }));

    // Pi reconstructs extensions on reload/new/resume/fork. Only process startup
    // checks; branding still applies to every replacement session.
    if (checked || event.reason !== "startup") return;
    checked = true;
    void checkLoomUpdate(exec, shutdown.signal).then((version) => {
      if (version && !shutdown.signal.aborted) {
        ctx.ui.notify(
          `Loom update available: ${version}. Run loom update or use the /loom skill to update.`,
          "info",
        );
      }
    });
  });

  pi.on("session_shutdown", () => shutdown.abort());
}

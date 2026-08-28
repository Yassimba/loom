#!/usr/bin/env python3
"""Worked example of a figure script: a state machine with guards and terminals.

Copy this shape per figure: arrows first, then labels, then nodes, then
callouts and the legend; one write() at the end. Run from anywhere:

    python3 example-figure.py /tmp/out/5-lifecycle
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw import *  # noqa: E402,F401

W, H = 144, 72
b = []
# --- transitions (paint before nodes) ---
b.append(hline(56, 156, 64, 156))
b.append(hline(208, 156, 336, 156)); b.append(label_above(272, 156, "Enter [has_skills]"))
b.append(elbow([(136, 120), (136, 80), (680, 80), (680, 120)])); b.append(label_above(408, 80, "Enter [!has_skills] · stage_visible skips Where"))
b.append(hline(480, 156, 608, 156)); b.append(label_above(544, 156, "Enter"))
b.append(hline(752, 156, 880, 156, color=ACCENT, marker="arrow-accent", width=1.4)); b.append(label_above(816, 156, "Enter · StartInstall", color=ACCENT))
b.append(vline(656, 192, 656, 412)); b.append(label_beside(648, 296, "Enter [nothing_chosen]", anchor="end"))
b.append(vline(952, 192, 952, 296)); b.append(label_beside(960, 244, "InstallEvent::Done"))
b.append(vline(952, 368, 952, 412)); b.append(label_beside(960, 388, "Enter · Esc · q"))
b.append(vline(136, 192, 136, 296)); b.append(label_beside(144, 244, "q · Esc [total_selected>0]"))
b.append(vline(136, 368, 136, 412)); b.append(label_beside(144, 388, "Enter · y · q"))
# --- states ---
b.append(start(48, 156))
b.append(state(64, 120, W, H, "Choose", "CHOOSE = 0"))
b.append(state(336, 120, W, H, "Where", "WHERE = 1 · optional"))
b.append(state(608, 120, W, H, "Review", "plan() Err → stay"))
b.append(state(880, 120, W, H, "Install(running)", "keys ignored · no way back", focal=True))
b.append(state(880, 296, W, H, "Install(done)", "report: Some(_)"))
b.append(state(64, 296, W, H, "confirm_quit", "other key → back", wait=True))
b.append(ring(136, 420, "Cancelled")); b.append(ring(656, 420, "NothingSelected")); b.append(ring(952, 420, "Installed(report)"))
# --- asides + legend ---
b.append(callout(336, 232, ["Esc / Back on Where or Review → previous", "visible stage (go_back). Esc on Choose → quit()."]))
b.append(legend(504, 1100, [(sw_box("step"), "STAGE"), (sw_box("focal"), "FOCAL STAGE"), (sw_box("store"), "WAIT STATE"),
                            (sw_start(), "START"), (sw_ring(), "OUTCOME"), (sw_line(color=ACCENT, marker="arrow-accent"), "POINT OF NO RETURN")]))

if __name__ == "__main__":
    out = write(sys.argv[1] if len(sys.argv) > 1 else "example-lifecycle", "State machine",
                "Choose → (Where) → Review → Install, never back",
                "State machine: stages advance Choose to Where to Review to Install; Where is skipped when no skill is selected; Install has no way back.",
                1100, 544, "\n".join(b), project="example")
    print("wrote", out, "and", out.with_suffix(".svg"))

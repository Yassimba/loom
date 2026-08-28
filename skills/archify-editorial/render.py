#!/usr/bin/env python3
"""Render an archify spec (any of its five types) in the diagram-design editorial skin.

archify owns the content model and the geometry: node kinds, lanes, frames,
validated orthogonal routes, and collision-free label rectangles.
diagram-design owns the look: paper/ink/accent tokens, Geist + Instrument
Serif, rounded elbows, masked labels, zone eyebrows, bottom legend strip.

Geometry comes from archify's own rendered SVG, which tags every node
(`data-node-id`), edge (`data-composition-points`), and frame
(`data-composition-frame-kind`). Untagged lines and rectangles are chrome:
lifelines, rails, activation bars.

Usage:
    render.py <spec.json> <out.html> [--focal id,id] [--dark] [--eyebrow TEXT]
"""

from __future__ import annotations

import argparse
import html
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from html.parser import HTMLParser
from pathlib import Path

# archify is a sibling skill: next to this one in the repo or agent tree, else the global tree.
_HERE = Path(__file__).resolve().parent
ARCHIFY = next(
    (p for p in (_HERE.parent / "archify/bin/archify.mjs", Path.home() / ".claude/skills/archify/bin/archify.mjs") if p.exists()),
    Path.home() / ".claude/skills/archify/bin/archify.mjs",
)

LIGHT = {
    "paper": "#f5f5f5", "paper2": "#ececec", "ink": "#2d3142", "muted": "#4f5d75",
    "soft": "#7a8399", "accent": "#eb6c36", "accent_tint": "rgba(235,108,54,0.08)",
    "link": "#2e5aa8", "ink_rgb": "45,49,66", "accent_rgb": "235,108,54", "white": "#ffffff",
}
DARK = {
    "paper": "#2d3142", "paper2": "#393e53", "ink": "#f5f5f5", "muted": "#bfc0c0",
    "soft": "#8e98ac", "accent": "#f08a59", "accent_tint": "rgba(240,138,89,0.10)",
    "link": "#6a95d8", "ink_rgb": "245,245,245", "accent_rgb": "240,138,89", "white": "#393e53",
}

# archify node kind -> (diagram-design treatment, eyebrow tag)
TREATMENT = {
    "frontend": ("backend", "UI"), "backend": ("backend", "SVC"), "database": ("store", "DB"),
    "messagebus": ("store", "BUS"), "cloud": ("external", "CLOUD"), "external": ("external", "EXT"),
    "security": ("security", "SEC"),
    # lifecycle states
    "start": ("input", "START"), "active": ("backend", "STATE"), "decision": ("backend", "GATE"),
    "waiting": ("external", "WAIT"), "success": ("store", "DONE"), "failure": ("security", "FAIL"),
    # workflow extras
    "process": ("backend", "STEP"), "end": ("store", "END"),
}
R = 8  # elbow radius, diagram-design mandatory


@dataclass
class El:
    tag: str
    attrs: dict[str, str]
    depth: int
    text: str = ""
    children: list[El] = field(default_factory=list)


class SvgTree(HTMLParser):
    """Collect the first `<svg viewBox>` of a document as a small element tree."""

    def __init__(self) -> None:
        super().__init__()
        self.root: El | None = None
        self.stack: list[El] = []
        self.done = False

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if self.done:
            return
        a = {k: v or "" for k, v in attrs}
        if self.root is None:
            if tag == "svg" and "viewbox" in a:
                self.root = El(tag, a, 0)
                self.stack = [self.root]
            return
        el = El(tag, a, len(self.stack))
        self.stack[-1].children.append(el)
        if tag not in ("rect", "line", "circle", "path", "polygon", "use", "stop"):
            self.stack.append(el)

    def handle_endtag(self, tag: str) -> None:
        if self.root is None or self.done or tag in ("rect", "line", "circle", "path", "polygon", "use", "stop"):
            return
        if self.stack and self.stack[-1].tag == tag:
            self.stack.pop()
        if not self.stack:
            self.done = True

    def handle_data(self, data: str) -> None:
        if self.root is not None and not self.done and self.stack and self.stack[-1].tag == "text":
            self.stack[-1].text += data


def archify_svg(kind: str, spec: Path, quality: str) -> El:
    check = subprocess.run(
        ["node", str(ARCHIFY), "validate", kind, str(spec), "--quality", quality, "--json"],
        capture_output=True, text=True, check=False,
    )
    try:
        receipt = json.loads(check.stdout)
    except json.JSONDecodeError:
        sys.exit(f"archify validate failed:\n{check.stdout}\n{check.stderr}")
    if not receipt.get("ok"):
        sys.exit(f"archify validation failed; fix the spec first:\n{receipt.get('error')}")
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "archify.html"
        run = subprocess.run(
            ["node", str(ARCHIFY), "render", kind, str(spec), str(out), "--quality", quality],
            capture_output=True, text=True, check=False,
        )
        if run.returncode != 0:
            sys.exit(f"archify render failed:\n{run.stdout}\n{run.stderr}")
        parser = SvgTree()
        parser.feed(out.read_text())
    if parser.root is None:
        sys.exit("archify output has no <svg viewBox>")
    return parser.root


def walk(el: El):
    for child in el.children:
        yield child
        yield from walk(child)


def num(el: El, key: str) -> float:
    return float(el.attrs.get(key, "0"))


def elbow_path(points: list[tuple[float, float]]) -> str:
    """Orthogonal polyline -> path with quarter-arc corners."""
    if len(points) < 3:
        return "M " + " L ".join(f"{x},{y}" for x, y in points)
    parts = [f"M {points[0][0]},{points[0][1]}"]
    for i in range(1, len(points) - 1):
        (px, py), (cx, cy), (nx, ny) = points[i - 1], points[i], points[i + 1]
        r = min(R, (abs(cx - px) + abs(cy - py)) / 2, (abs(nx - cx) + abs(ny - cy)) / 2)
        sgn = lambda a, b: (a > b) - (a < b)  # noqa: E731
        ix, iy = cx - r * sgn(cx, px), cy - r * sgn(cy, py)
        ox, oy = cx + r * sgn(nx, cx), cy + r * sgn(ny, cy)
        parts.append(f"L {ix},{iy} Q {cx},{cy} {ox},{oy}")
    parts.append(f"L {points[-1][0]},{points[-1][1]}")
    return " ".join(parts)


def parse_points(s: str) -> list[tuple[float, float]]:
    return [(float(x), float(y)) for x, y in (p.split(",") for p in s.split(";"))]


def node_style(kind: str, t: dict, focal: bool) -> tuple[str, str, str]:
    """Return fill, stroke, dasharray for one diagram-design node treatment."""
    if focal:
        return t["accent_tint"], t["accent"], ""
    ink, acc = t["ink_rgb"], t["accent_rgb"]
    return {
        "backend": (t["white"], t["ink"], ""),
        "store": (f"rgba({ink},0.05)", t["muted"], ""),
        "external": (f"rgba({ink},0.03)", f"rgba({ink},0.30)", ""),
        "input": (f"rgba({ink},0.06)", t["soft"], ""),
        "security": (f"rgba({acc},0.05)", f"rgba({acc},0.50)", "4,4"),
    }[kind]


def mono(x: float, y: float, text: str, fill: str, size: int = 8, anchor: str = "middle", track: str = "0.06em") -> str:
    return (
        f'<text x="{x:g}" y="{y:g}" fill="{fill}" font-size="{size}" font-family="\'Geist Mono\', monospace" '
        f'text-anchor="{anchor}" letter-spacing="{track}">{html.escape(text)}</text>'
    )


def render(spec: dict, svg_root: El, *, focal: set[str], dark: bool, eyebrow: str) -> str:
    t = DARK if dark else LIGHT
    ink = t["ink_rgb"]
    vb = [float(v) for v in svg_root.attrs["viewbox"].split()]
    vb_w, vb_h = vb[2], vb[3]
    zones: list[str] = []
    chrome: list[str] = []
    edges: list[str] = []
    labels: list[str] = []
    nodes: list[str] = []
    present: list[str] = []

    node_groups = [el for el in walk(svg_root) if "data-node-id" in el.attrs]
    inside_nodes: set[int] = {id(d) for g in node_groups for d in walk(g)}
    # archify draws its own legend; diagram-design gets one strip at the bottom instead
    for legend in [el for el in walk(svg_root) if "data-legend" in el.attrs]:
        inside_nodes.add(id(legend))
        inside_nodes.update(id(d) for d in walk(legend))
    top_level = [el for el in walk(svg_root) if id(el) not in inside_nodes and "data-node-id" not in el.attrs]

    pending_mask: El | None = None
    mask_used = False
    for el in top_level:
        a = el.attrs
        if el.tag != "text" and not (el.tag == "rect" and "c-mask" in a.get("class", "")):
            pending_mask = None
        if el.tag == "rect" and "data-composition-frame-kind" in a:
            secure = "exception" in a["data-composition-frame-kind"] or "security" in a.get("class", "")
            stroke = f"rgba({t['accent_rgb']},0.50)" if secure else f"rgba({ink},0.10)"
            fill = f"rgba({t['accent_rgb']},0.03)" if secure else f"rgba({ink},0.02)"
            dash = ' stroke-dasharray="4,4"' if secure else ""
            zones.append(
                f'<rect x="{num(el, "x"):g}" y="{num(el, "y"):g}" width="{num(el, "width"):g}" height="{num(el, "height"):g}" '
                f'rx="8" fill="{fill}" stroke="{stroke}" stroke-width="0.8"{dash}/>'
            )
        elif "data-composition-points" in a:
            cls = a.get("class", "")
            stroke, width, dash, marker = t["muted"], "1.2", "", "arrow"
            if "a-emphasis" in cls:
                stroke, width, marker = t["ink"], "1.4", "arrow-ink"
            elif "a-security" in cls:
                stroke, dash, marker = t["accent"], ' stroke-dasharray="4,4"', "arrow-accent"
            if a.get("stroke-dasharray"):
                width, dash = "1", ' stroke-dasharray="4,3"'
            head = f' marker-end="url(#{marker})"' if a.get("marker-end") else ""
            edges.append(
                f'<path d="{elbow_path(parse_points(a["data-composition-points"]))}" fill="none" '
                f'stroke="{stroke}" stroke-width="{width}"{dash}{head}/>'
            )
        elif el.tag == "rect" and "c-mask" in a.get("class", ""):
            pending_mask = el
            mask_used = False
        elif el.tag == "rect" and "c-grid" not in a.get("class", "") and a.get("width") not in ("100%", None):
            # activation bar or other untagged box
            chrome.append(
                f'<rect x="{num(el, "x"):g}" y="{num(el, "y"):g}" width="{num(el, "width"):g}" height="{num(el, "height"):g}" '
                f'rx="2" fill="rgba({ink},0.08)" stroke="{t["muted"]}" stroke-width="0.8"/>'
            )
        elif el.tag in ("path", "line") and "c-grid" not in a.get("class", ""):
            dash = ' stroke-dasharray="3,5"' if a.get("stroke-dasharray") else ""
            cls = a.get("class", "")
            stroke = t["ink"] if "a-emphasis" in cls else f"rgba({ink},0.35)"
            head = ' marker-end="url(#arrow-ink)"' if a.get("marker-end") and "a-emphasis" in cls else (
                ' marker-end="url(#arrow)"' if a.get("marker-end") else ""
            )
            geom = f'd="{a["d"]}"' if el.tag == "path" else (
                f'x1="{a.get("x1")}" y1="{a.get("y1")}" x2="{a.get("x2")}" y2="{a.get("y2")}"'
            )
            chrome.append(f'<{el.tag} {geom} fill="none" stroke="{stroke}" stroke-width="1"{dash}{head}/>')
        elif el.tag == "text" and el.text.strip():
            text = el.text.strip()
            x, y = num(el, "x"), num(el, "y")
            anchor = a.get("text-anchor", "start")
            if pending_mask is not None:
                m = pending_mask
                up = text.upper()
                w = max(num(m, "width"), len(up) * 5.2 + 8)
                cx = num(m, "x") + num(m, "width") / 2
                if not mask_used:
                    labels.append(
                        f'<rect x="{cx - w / 2:g}" y="{num(m, "y"):g}" width="{w:g}" height="{num(m, "height"):g}" rx="2" fill="{t["paper"]}"/>'
                    )
                size = 8 if not mask_used else 7
                labels.append(mono(x if anchor == "middle" else cx, y, up, t["soft"], size))
                mask_used = True
            else:
                # lane / stage / segment eyebrow
                labels.append(mono(x, y, text.upper(), f"rgba({ink},0.45)", 7, anchor, "0.04em"))

    sublabels = {c["id"]: c for c in spec.get("components", spec.get("nodes", spec.get("states", spec.get("participants", []))))}
    for g in node_groups:
        a = g.attrs
        box = next((c for c in g.children if c.tag == "rect"), None)
        if box is None:
            continue
        kind, tag = TREATMENT.get(a.get("data-node-kind", "backend"), ("backend", a.get("data-node-kind", "")[:5].upper()))
        if kind not in present:
            present.append(kind)
        fill, stroke, dasharray = node_style(kind, t, a["data-node-id"] in focal)
        dash = f' stroke-dasharray="{dasharray}"' if dasharray else ""
        x, y, w, h = num(box, "x"), num(box, "y"), num(box, "width"), num(box, "height")
        cx, cy = x + w / 2, y + h / 2
        tag_w = len(tag) * 6 + 12
        sub = a.get("data-node-sublabel", "")
        name_y = cy + 5 if sub else cy + 4
        size = 12 if w >= 110 else 11
        nodes.append(
            f'<rect x="{x:g}" y="{y:g}" width="{w:g}" height="{h:g}" rx="6" fill="{t["paper"]}"/>'
            f'<rect x="{x:g}" y="{y:g}" width="{w:g}" height="{h:g}" rx="6" fill="{fill}" stroke="{stroke}" stroke-width="1"{dash}/>'
            f'<rect x="{x + 8:g}" y="{y + 6:g}" width="{tag_w}" height="12" rx="2" fill="transparent" stroke="{stroke}" stroke-opacity="0.4" stroke-width="0.8"/>'
            + mono(x + 8 + tag_w / 2, y + 15, tag, stroke, 7, "middle", "0.08em").replace('font-size="7"', 'font-size="7" fill-opacity="0.8"')
            + f'<text x="{cx:g}" y="{name_y:g}" fill="{t["ink"]}" font-size="{size}" font-weight="600" '
            f'font-family="\'Geist\', sans-serif" text-anchor="middle">{html.escape(a["data-node-label"])}</text>'
        )
        if sub:
            nodes.append(mono(cx, cy + 19, sub, t["muted"], 8, "middle", "0"))

    # legend strip
    legend_y = vb_h + 24
    legend = [
        f'<line x1="30" y1="{legend_y - 8:g}" x2="{vb_w - 30:g}" y2="{legend_y - 8:g}" stroke="rgba({ink},0.10)" stroke-width="0.8"/>',
        mono(30, legend_y + 8, "LEGEND", t["muted"], 8, "start", "0.14em"),
    ]
    lx = 110
    for kind in present + (["focal"] if focal else []):
        fill, stroke, dasharray = (t["accent_tint"], t["accent"], "") if kind == "focal" else node_style(kind, t, False)
        dash = f' stroke-dasharray="{dasharray}"' if dasharray else ""
        legend.append(
            f'<rect x="{lx}" y="{legend_y - 1:g}" width="20" height="12" rx="2" fill="{fill}" stroke="{stroke}" stroke-width="0.8"{dash}/>'
            + mono(lx + 28, legend_y + 8, kind.upper(), t["muted"], 8, "start")
        )
        lx += 120

    title = spec["meta"]["title"]
    cards = "".join(
        f'<section class="card"><h2><span class="dot"></span>{html.escape(c["title"])}</h2><ul>'
        + "".join(f"<li>{html.escape(i)}</li>" for i in c.get("items", []))
        + "</ul></section>"
        for c in spec.get("cards", [])
    )
    desc = f"{spec['diagram_type'].capitalize()} diagram with {len(node_groups)} nodes and {len(edges)} connections."
    body = "\n".join(zones + chrome + edges + labels + nodes + legend)
    e = html.escape
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{e(title)}</title>
<link href="https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500;600&display=swap" rel="stylesheet">
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
:root{{--paper:{t["paper"]};--paper2:{t["paper2"]};--ink:{t["ink"]};--muted:{t["muted"]};--accent:{t["accent"]};--rule:rgba({ink},0.12)}}
body{{font-family:'Geist',system-ui,sans-serif;background:var(--paper);color:var(--ink);padding:3rem 2rem}}
.frame{{max-width:1280px;margin:0 auto}}
.eyebrow{{font-family:'Geist Mono',monospace;font-size:.66rem;font-weight:500;letter-spacing:.18em;text-transform:uppercase;color:var(--muted);margin-bottom:.5rem}}
h1{{font-family:'Instrument Serif',serif;font-size:clamp(1.5rem,2.4vw + .75rem,2rem);font-weight:400;letter-spacing:-.02em;line-height:1.15;margin-bottom:1.5rem}}
svg{{width:100%;display:block}}
.cards{{display:grid;grid-template-columns:1.1fr 1fr .9fr;gap:1rem;margin-top:2rem}}
.card{{border:1px solid var(--rule);border-radius:8px;padding:1rem 1.125rem;background:var(--paper2)}}
.card h2{{font-family:'Geist',sans-serif;font-size:.85rem;font-weight:600;margin-bottom:.5rem;display:flex;align-items:center;gap:.5rem}}
.dot{{width:6px;height:6px;border-radius:50%;background:var(--accent);display:inline-block}}
.card li{{font-size:.8rem;color:var(--muted);line-height:1.45;margin-left:1rem;margin-bottom:.25rem}}
footer{{margin-top:2rem;padding-top:.75rem;border-top:1px solid var(--rule);font-family:'Geist Mono',monospace;font-size:.6rem;letter-spacing:.1em;color:var(--muted)}}
@media (max-width:900px){{.cards{{grid-template-columns:1fr}}}}
</style>
</head>
<body>
<div class="frame">
<p class="eyebrow">{e(eyebrow)}</p>
<h1>{e(title)}</h1>
<svg viewBox="0 0 {vb_w:g} {legend_y + 36:g}" xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="d-title d-desc">
<title id="d-title">{e(title)}</title>
<desc id="d-desc">{e(desc)}</desc>
<defs>
<marker id="arrow" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{t["muted"]}"/></marker>
<marker id="arrow-ink" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{t["ink"]}"/></marker>
<marker id="arrow-accent" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{t["accent"]}"/></marker>
<marker id="arrow-link" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{t["link"]}"/></marker>
</defs>
<rect width="100%" height="100%" fill="{t["paper"]}"/>
{body}
</svg>
<div class="cards">{cards}</div>
<footer>archify {e(spec["diagram_type"])} spec · diagram-design skin · {len(node_groups)} nodes · {len(edges)} connections</footer>
</div>
</body>
</html>
"""


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("spec", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--focal", default="", help="comma-separated node ids to accent (max 2)")
    ap.add_argument("--dark", action="store_true")
    ap.add_argument("--eyebrow", default="")
    args = ap.parse_args()
    focal = {f for f in args.focal.split(",") if f}
    if len(focal) > 2:
        sys.exit("diagram-design allows at most 2 focal nodes")
    spec = json.loads(args.spec.read_text())
    kind = spec.get("diagram_type", "")
    if kind not in ("architecture", "workflow", "sequence", "dataflow", "lifecycle"):
        sys.exit(f"unknown archify diagram_type {kind!r}")
    quality = spec.get("meta", {}).get("quality_profile", "standard")
    svg_root = archify_svg(kind, args.spec, quality)
    eyebrow = args.eyebrow or f"{kind.capitalize()} · Diagram Design"
    args.out.write_text(render(spec, svg_root, focal=focal, dark=args.dark, eyebrow=eyebrow))
    print(f"wrote {args.out}")


if __name__ == "__main__":
    main()

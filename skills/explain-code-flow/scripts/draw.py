#!/usr/bin/env python3
"""Drawing kit for explain-code-flow figures.

Primitives that already satisfy diagram-design's default profile (paper/ink/
muted/coral palette, Geist + Geist Mono + Instrument Serif, 4px grid, masked
labels, orthogonal connectors, paint order background -> zones -> arrows ->
labels -> nodes). A figure script imports this module, composes a body from
the primitives, and calls `write()` which emits `<stem>.html` and `<stem>.svg`.

    from draw import *
    b = []
    b.append(hline(184, 92, 280, 92)); b.append(label_above(232, 92, "Model"))
    b.append(node(40, 64, 144, 56, "run_loop", "wizard/mod.rs:40", kind="focal", tag="UI THREAD", mono=True))
    write("diagrams/1-overview", "Architecture", "One UI thread, two workers",
          "Architecture diagram: ...", 1160, 544, "\n".join(b), project="loom TUI")

Coordinates are in viewBox units. Keep every x/y/width/height on the 4px grid
(`r4`). Draw arrows before the nodes they touch; put a label's mask on a free
segment of its connector, never under a node drawn later.

Primitives
----------
Nodes:      node, cls (UML class), state, diamond, oval, step, ring (terminal),
            start (initial dot), participant (sequence header)
Connectors: hline, vline, path (raw d), elbow (orthogonal polyline from points)
Labels:     label_above (horizontal segment), label_beside (vertical segment),
            mult (UML multiplicity)
Containers: zone, fragment (sequence LOOP/OPT/ALT), lifeline, activation
Chrome:     callout, legend + swatches (sw_box, sw_line, sw_ring, sw_diamond),
            page, write
"""
from __future__ import annotations

import html
import re
from pathlib import Path

PAPER, INK, MUTED, SOFT, ACCENT, LINK = "#f5f5f5", "#2d3142", "#4f5d75", "#7a8399", "#eb6c36", "#2e5aa8"
ADDED, REMOVED, CHANGED = "#2f7d4f", "#b3382c", "#b7791f"  # diff mode (references/diagram-diff.md)
MONO = "'Geist Mono', monospace"
SANS = "'Geist', sans-serif"
SERIF = "'Instrument Serif', serif"
FONTS_HREF = "https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500;600&display=swap"

# node kinds: fill, stroke
KIND = {
    "focal": ("rgba(235,108,54,0.08)", ACCENT),      # 1-2 per figure
    "step": ("#ffffff", INK),                          # function / component
    "store": ("rgba(45,49,66,0.05)", MUTED),           # state, storage, wait state
    "external": ("rgba(45,49,66,0.03)", "rgba(45,49,66,0.30)"),
    "input": ("rgba(79,93,117,0.10)", SOFT),           # entry / CLI / user
    "async": ("rgba(45,49,66,0.02)", "rgba(45,49,66,0.20)"),  # dashed: thread, job
}


def r4(v: float) -> int:
    return int(round(v / 4.0)) * 4


def esc(text: str) -> str:
    """Escape for SVG text. Pass already-escaped text through unchanged."""
    return text if re.search(r"&(amp|lt|gt|quot|#\d+);", text) else html.escape(text, quote=False)


def mono_w(text: str, size: int = 8) -> int:
    """Mask width for a mono label of `size` px, on the 8px grid."""
    return int(round((len(text) * size * 0.68 + 8) / 8.0)) * 8


# ---------------------------------------------------------------- nodes
def node(x, y, w, h, name, sub=None, kind="step", tag=None, mono=False, tw=None):
    """Box with optional eyebrow tag ("EXT", "FN", "THREAD") and mono sublabel."""
    fill, stroke = KIND[kind]
    dash = ' stroke-dasharray="4,3"' if kind == "async" else ""
    tag_col = ACCENT if kind == "focal" else MUTED
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{PAPER}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{fill}" stroke="{stroke}" stroke-width="1"{dash}/>']
    cy = y + h // 2
    if tag:
        tw = tw or r4(len(tag) * 5.2 + 12)
        s.append(f'<rect x="{x+8}" y="{y+6}" width="{tw}" height="12" rx="2" fill="transparent" stroke="{tag_col}" stroke-opacity="0.4" stroke-width="0.8"/>')
        s.append(f'<text x="{x+8+tw//2}" y="{y+15}" fill="{tag_col}" font-size="7" font-family="{MONO}" text-anchor="middle" letter-spacing="0.08em">{esc(tag)}</text>')
        cy += 4
    fam = MONO if mono else SANS
    ny = cy - 2 if sub else cy + 4
    s.append(f'<text x="{x+w//2}" y="{ny}" fill="{INK}" font-size="12" font-weight="600" font-family="{fam}" text-anchor="middle">{esc(name)}</text>')
    if sub:
        s.append(f'<text x="{x+w//2}" y="{ny+14}" fill="{MUTED}" font-size="9" font-family="{MONO}" text-anchor="middle">{esc(sub)}</text>')
    return "\n".join(s)


def participant(x, y, w, h, name, sub=None, kind="step", tag=None):
    """Sequence-diagram header box; returns (svg, center_x)."""
    return node(x, y, w, h, name, sub, kind=kind, tag=tag, mono=True), x + w // 2


def state(x, y, w, h, name, sub=None, focal=False, wait=False):
    kind = "focal" if focal else ("store" if wait else "step")
    return node(x, y, w, h, name, sub, kind=kind, tag="WAIT" if wait else "STAGE", mono=True)


def start(cx, cy):
    return f'<circle cx="{cx}" cy="{cy}" r="6" fill="{INK}"/>'


def ring(cx, cy, label):
    """Terminal state: bullseye with a label under it."""
    return "\n".join([
        f'<circle cx="{cx}" cy="{cy}" r="8" fill="none" stroke="{INK}" stroke-width="1"/>',
        f'<circle cx="{cx}" cy="{cy}" r="5" fill="{INK}"/>',
        f'<text x="{cx}" y="{cy+24}" fill="{INK}" font-size="12" font-weight="600" font-family="{MONO}" text-anchor="middle">{esc(label)}</text>'])


def diamond(cx, cy, text, focal=False, sub=None, hw=112, hh=40):
    """Decision node centered on (cx, cy); tips at cx±hw, cy±hh."""
    fill, stroke = ("rgba(235,108,54,0.08)", ACCENT) if focal else ("#ffffff", INK)
    pts = f"{cx},{cy-hh} {cx+hw},{cy} {cx},{cy+hh} {cx-hw},{cy}"
    s = [f'<polygon points="{pts}" fill="{PAPER}"/>',
         f'<polygon points="{pts}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>']
    ty = cy - 2 if sub else cy + 4
    s.append(f'<text x="{cx}" y="{ty}" fill="{INK}" font-size="12" font-weight="600" font-family="{MONO}" text-anchor="middle">{esc(text)}</text>')
    if sub:
        s.append(f'<text x="{cx}" y="{ty+14}" fill="{MUTED}" font-size="9" font-family="{MONO}" text-anchor="middle">{esc(sub)}</text>')
    return "\n".join(s)


def oval(x, y, w, h, text, sub=None):
    """Entry / exit pill."""
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="24" fill="{PAPER}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="24" fill="rgba(45,49,66,0.03)" stroke="rgba(45,49,66,0.30)" stroke-width="1"/>']
    cy = y + h // 2
    ty = cy - 2 if sub else cy + 4
    s.append(f'<text x="{x+w//2}" y="{ty}" fill="{INK}" font-size="12" font-weight="600" font-family="{MONO}" text-anchor="middle">{esc(text)}</text>')
    if sub:
        s.append(f'<text x="{x+w//2}" y="{ty+14}" fill="{MUTED}" font-size="9" font-family="{MONO}" text-anchor="middle">{esc(sub)}</text>')
    return "\n".join(s)


def step(x, y, w, h, lines, eyebrow=None):
    """Left-aligned multi-line action box (flowchart process)."""
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{PAPER}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="#ffffff" stroke="{INK}" stroke-width="1"/>']
    if eyebrow:
        s.append(f'<text x="{x+8}" y="{y-8}" fill="{MUTED}" font-size="8" font-family="{MONO}" letter-spacing="0.08em">{esc(eyebrow)}</text>')
    for i, t in enumerate(lines):
        s.append(f'<text x="{x+16}" y="{y+20+16*i}" fill="{INK}" font-size="9" font-family="{MONO}">{esc(t)}</text>')
    return "\n".join(s)


def cls(x, y, w, name, attrs, ops=None, stereotype=None, focal=False):
    """UML class box. Returns (svg, height). Rows are 20px; a row of "…" is muted."""
    name_h = 40
    ah = len(attrs) * 20 + 8 if attrs else 0
    oh = len(ops) * 20 + 8 if ops else 0
    h = name_h + ah + oh
    stroke = ACCENT if focal else INK
    fill = "rgba(235,108,54,0.04)" if focal else "#ffffff"
    hair = "rgba(235,108,54,0.40)" if focal else "rgba(45,49,66,0.22)"
    s = [f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{PAPER}"/>',
         f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="6" fill="{fill}" stroke="{stroke}" stroke-width="1"/>']
    if focal:
        s.append(f'<rect x="{x}" y="{y}" width="{w}" height="{name_h}" rx="6" fill="rgba(235,108,54,0.10)"/>')
        s.append(f'<rect x="{x}" y="{y+name_h-8}" width="{w}" height="8" fill="rgba(235,108,54,0.10)"/>')
    cx = x + w // 2
    if stereotype:
        s.append(f'<text x="{cx}" y="{y+16}" fill="{MUTED}" font-size="8" font-family="{MONO}" text-anchor="middle" letter-spacing="0.08em">«{esc(stereotype)}»</text>')
        s.append(f'<text x="{cx}" y="{y+32}" fill="{INK}" font-size="12" font-weight="600" font-family="{MONO}" text-anchor="middle">{esc(name)}</text>')
    else:
        s.append(f'<text x="{cx}" y="{y+24}" fill="{INK}" font-size="12" font-weight="600" font-family="{MONO}" text-anchor="middle">{esc(name)}</text>')
    yy = y + name_h
    for comp in (attrs, ops):
        if not comp:
            continue
        s.append(f'<line x1="{x}" y1="{yy}" x2="{x+w}" y2="{yy}" stroke="{hair}" stroke-width="1"/>')
        for i, t in enumerate(comp):
            col = MUTED if t.strip() == "…" else INK
            s.append(f'<text x="{x+16}" y="{yy+20+20*i}" fill="{col}" font-size="9" font-family="{MONO}">{esc(t)}</text>')
        yy += len(comp) * 20 + 8
    return "\n".join(s), h


# ----------------------------------------------------------- connectors
def arrow_attrs(color=MUTED, dashed=False, marker="arrow", width=1.2):
    d = ' stroke-dasharray="5,4"' if dashed else ""
    m = f' marker-end="url(#{marker})"' if marker else ""
    return f'fill="none" stroke="{color}" stroke-width="{width}"{d}{m}'


def hline(x1, y, x2, y2=None, **kw):
    """Horizontal (or any straight) connector. `y2` defaults to `y`."""
    y2 = y if y2 is None else y2
    return f'<path d="M {x1},{y} L {x2},{y2}" {arrow_attrs(**kw)}/>'


vline = hline  # same signature: vline(x, y1, x, y2)


def line(x1, y1, x2, y2, **kw):
    return hline(x1, y1, x2, y2, **kw)


def path(d, **kw):
    return f'<path d="{d}" {arrow_attrs(**kw)}/>'


def elbow(points, r=8, **kw):
    """Orthogonal polyline through `points` [(x,y), ...] with rounded corners.
    Consecutive points must share an x or a y."""
    d = [f"M {points[0][0]},{points[0][1]}"]
    for i in range(1, len(points) - 1):
        (px, py), (cx, cy), (nx, ny) = points[i - 1], points[i], points[i + 1]
        dx1 = 0 if cx == px else (1 if cx > px else -1)
        dy1 = 0 if cy == py else (1 if cy > py else -1)
        dx2 = 0 if nx == cx else (1 if nx > cx else -1)
        dy2 = 0 if ny == cy else (1 if ny > cy else -1)
        d.append(f"L {cx - dx1*r},{cy - dy1*r}")
        d.append(f"Q {cx},{cy} {cx + dx2*r},{cy + dy2*r}")
    d.append(f"L {points[-1][0]},{points[-1][1]}")
    return path(" ".join(d), **kw)


def uml(d, marker_end=None, marker_start=None, dashed=False, color=INK):
    """UML relationship stroke; markers: uml-triangle, uml-diamond-filled,
    uml-diamond-hollow, uml-open."""
    ms = f' marker-start="url(#{marker_start})"' if marker_start else ""
    me = f' marker-end="url(#{marker_end})"' if marker_end else ""
    dd = ' stroke-dasharray="4,3"' if dashed else ""
    return f'<path d="{d}" fill="none" stroke="{color}" stroke-width="1"{dd}{ms}{me}/>'


# --------------------------------------------------------------- labels
def label_above(cx, line_y, text, color=SOFT, lines=None):
    """Mono label centered on a horizontal segment, mask 8px above the stroke."""
    lines = lines or [text]
    w = max(mono_w(t) for t in lines)
    h = 12 * len(lines)
    top = line_y - 8 - h
    s = [f'<rect x="{cx - w//2}" y="{top}" width="{w}" height="{h}" rx="2" fill="{PAPER}"/>']
    for i, t in enumerate(lines):
        s.append(f'<text x="{cx}" y="{top + 9 + 12*i}" fill="{color}" font-size="8" font-family="{MONO}" text-anchor="middle" letter-spacing="0.06em">{esc(t)}</text>')
    return "\n".join(s)


def label_beside(x_edge, cy, text, color=SOFT, lines=None, anchor="start"):
    """Mono label beside a vertical segment. anchor="start": x_edge is the mask's
    left edge (>= line_x + 8); anchor="end": x_edge is its right edge (<= line_x - 8)."""
    lines = lines or [text]
    w = max(mono_w(t) for t in lines)
    h = 12 * len(lines)
    top = r4(cy - h / 2)
    x_left = x_edge - w if anchor == "end" else x_edge
    s = [f'<rect x="{x_left}" y="{top}" width="{w}" height="{h}" rx="2" fill="{PAPER}"/>']
    for i, t in enumerate(lines):
        s.append(f'<text x="{x_left + 4}" y="{top + 9 + 12*i}" fill="{color}" font-size="8" font-family="{MONO}" letter-spacing="0.06em">{esc(t)}</text>')
    return "\n".join(s)


def mult(cx, cy, text):
    """UML multiplicity chip."""
    w = mono_w(text)
    return (f'<rect x="{cx-w//2}" y="{cy-6}" width="{w}" height="12" rx="2" fill="{PAPER}"/>'
            f'<text x="{cx}" y="{cy+3}" fill="{INK}" font-size="8" font-family="{MONO}" text-anchor="middle" font-weight="600">{esc(text)}</text>')


# ----------------------------------------------------------- containers
def zone(x, y, w, h, label, accent=False):
    """Boundary container with an eyebrow. Paint before arrows."""
    lw = r4(len(label) * 5.2 + 16)
    if accent:
        box = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="rgba(235,108,54,0.05)" stroke="rgba(235,108,54,0.50)" stroke-width="0.8" stroke-dasharray="4,4"/>'
        col = "rgba(235,108,54,0.80)"
    else:
        box = f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="rgba(45,49,66,0.02)" stroke="rgba(45,49,66,0.10)" stroke-width="0.8"/>'
        col = "rgba(45,49,66,0.40)"
    return "\n".join([box,
        f'<rect x="{x+12}" y="{y+4}" width="{lw}" height="12" rx="2" fill="{PAPER}"/>',
        f'<text x="{x+12+lw//2}" y="{y+13}" fill="{col}" font-size="7" font-family="{MONO}" text-anchor="middle" letter-spacing="0.14em">{esc(label)}</text>'])


def lifeline(x, top, bottom):
    return f'<line x1="{x}" y1="{top}" x2="{x}" y2="{bottom}" stroke="rgba(45,49,66,0.20)" stroke-width="1" stroke-dasharray="3,3"/>'


def activation(x, top, bottom, w=8):
    """Activation bar centered on lifeline x."""
    return f'<rect x="{x - w//2}" y="{top}" width="{w}" height="{bottom-top}" fill="rgba(45,49,66,0.06)" stroke="{MUTED}" stroke-width="0.8"/>'


def fragment(x, y, w, h, op, guard):
    """Sequence fragment (LOOP / OPT / ALT). Paint after lifelines, before messages."""
    return "\n".join([
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="4" fill="rgba(45,49,66,0.02)" stroke="rgba(45,49,66,0.22)" stroke-width="1"/>',
        f'<rect x="{x}" y="{y}" width="44" height="16" rx="2" fill="{PAPER}" stroke="rgba(45,49,66,0.22)" stroke-width="1"/>',
        f'<text x="{x+22}" y="{y+12}" fill="{MUTED}" font-size="8" font-family="{MONO}" text-anchor="middle" letter-spacing="0.12em">{esc(op)}</text>',
        f'<text x="{x+56}" y="{y+12}" fill="{MUTED}" font-size="8" font-family="{MONO}" letter-spacing="0.04em">{esc(guard)}</text>'])


# --------------------------------------------------------------- chrome
def callout(x, y, lines):
    """Italic serif aside. At most two per figure."""
    return "\n".join(
        f'<text x="{x}" y="{y + 18*i}" fill="{MUTED}" font-size="14" font-style="italic" font-family="{SERIF}">{esc(t)}</text>'
        for i, t in enumerate(lines))


def legend(y, width, items):
    """Horizontal legend strip at baseline y. items: [(swatch_fn(x, y) -> svg, LABEL)]."""
    s = [f'<line x1="30" y1="{y-8}" x2="{width-30}" y2="{y-8}" stroke="rgba(45,49,66,0.10)" stroke-width="0.8"/>',
         f'<text x="30" y="{y+8}" fill="{MUTED}" font-size="8" font-family="{MONO}" letter-spacing="0.14em">LEGEND</text>']
    x = 100
    for sw, lab in items:
        s.append(sw(x, y))
        s.append(f'<text x="{x+24}" y="{y+8}" fill="{MUTED}" font-size="8" font-family="{MONO}" letter-spacing="0.04em">{esc(lab)}</text>')
        x += r4(28 + len(lab) * 5.4 + 24)
    return "\n".join(s)


def sw_box(kind):
    fill, stroke = KIND[kind]
    dash = ' stroke-dasharray="3,2"' if kind == "async" else ""
    return lambda x, y: f'<rect x="{x}" y="{y}" width="16" height="12" rx="2" fill="{fill}" stroke="{stroke}" stroke-width="1"{dash}/>'


def sw_line(color=MUTED, dashed=False, marker="arrow"):
    return lambda x, y: hline(x, y + 6, x + 16, color=color, dashed=dashed, marker=marker)


def sw_uml(marker_end=None, marker_start=None, dashed=False):
    return lambda x, y: uml(f"M {x},{y+6} L {x+20},{y+6}", marker_end, marker_start, dashed)


def sw_ring():
    return lambda x, y: f'<circle cx="{x+8}" cy="{y+6}" r="6" fill="none" stroke="{INK}"/><circle cx="{x+8}" cy="{y+6}" r="3" fill="{INK}"/>'


def sw_start():
    return lambda x, y: f'<circle cx="{x+8}" cy="{y+6}" r="5" fill="{INK}"/>'


def sw_diamond(focal=False):
    fill, stroke = ("rgba(235,108,54,0.08)", ACCENT) if focal else ("#ffffff", INK)
    return lambda x, y: f'<polygon points="{x+8},{y} {x+16},{y+6} {x+8},{y+12} {x},{y+6}" fill="{fill}" stroke="{stroke}" stroke-width="1"/>'


def sw_oval():
    return lambda x, y: f'<rect x="{x}" y="{y}" width="16" height="12" rx="6" fill="rgba(45,49,66,0.03)" stroke="rgba(45,49,66,0.30)"/>'


MARKERS = f'''
        <marker id="arrow" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{MUTED}"/></marker>
        <marker id="arrow-accent" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{ACCENT}"/></marker>
        <marker id="arrow-link" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{LINK}"/></marker>
        <marker id="arrow-open" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polyline points="0 0, 8 3, 0 6" fill="none" stroke="{MUTED}" stroke-width="1.2"/></marker>
        <marker id="uml-triangle" markerWidth="16" markerHeight="12" refX="15" refY="6" orient="auto" markerUnits="userSpaceOnUse"><polygon points="0 0, 16 6, 0 12" fill="{PAPER}" stroke="{INK}" stroke-width="1"/></marker>
        <marker id="uml-diamond-filled" markerWidth="18" markerHeight="10" refX="0" refY="5" orient="auto" markerUnits="userSpaceOnUse"><polygon points="0 5, 9 0, 18 5, 9 10" fill="{INK}"/></marker>
        <marker id="uml-diamond-hollow" markerWidth="18" markerHeight="10" refX="0" refY="5" orient="auto" markerUnits="userSpaceOnUse"><polygon points="0 5, 9 0, 18 5, 9 10" fill="{PAPER}" stroke="{INK}" stroke-width="1"/></marker>
        <marker id="uml-open" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto" markerUnits="userSpaceOnUse"><path d="M0,0 L9,4 L0,8" fill="none" stroke="{MUTED}" stroke-width="1"/></marker>
        <marker id="arrow-added" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{ADDED}"/></marker>
        <marker id="arrow-removed" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{REMOVED}"/></marker>
        <marker id="arrow-changed" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto"><polygon points="0 0, 8 3, 0 6" fill="{CHANGED}"/></marker>
'''


def page(slug, eyebrow, title, desc, vw, vh, body, project="", min_width=900):
    """Full diagram-design HTML page around one SVG."""
    tail = f" · {html.escape(project)}" if project else ""
    return f'''<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{html.escape(title)}</title>
  <link href="{FONTS_HREF}" rel="stylesheet">
  <style>
    *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
    :root {{
      --color-paper:   {PAPER};
      --color-ink:     {INK};
      --color-muted:   {MUTED};
      --color-accent:  {ACCENT};
      --font-sans:     'Geist', system-ui, sans-serif;
      --font-serif:    'Instrument Serif', serif;
      --font-mono:     'Geist Mono', ui-monospace, monospace;
    }}
    body {{ font-family: var(--font-sans); background: var(--color-paper); color: var(--color-ink); min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 3rem 2rem; }}
    .frame {{ max-width: 1200px; width: 100%; }}
    .eyebrow {{ font-family: var(--font-mono); font-size: 0.66rem; font-weight: 500; letter-spacing: 0.18em; text-transform: uppercase; color: var(--color-muted); margin-bottom: 0.5rem; }}
    h1 {{ font-family: var(--font-serif); font-size: clamp(1.5rem, 2.4vw + 0.75rem, 2rem); font-weight: 400; letter-spacing: -0.02em; line-height: 1.15; color: var(--color-ink); margin-bottom: 1.5rem; }}
    .scroll {{ overflow-x: auto; }}
    svg {{ width: 100%; min-width: {min_width}px; display: block; }}
  </style>
</head>
<body>
  <div class="frame">
    <p class="eyebrow">{html.escape(eyebrow)}{tail}</p>
    <h1>{html.escape(title)}</h1>
    <div class="scroll">
    <svg viewBox="0 0 {vw} {vh}" xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="{slug}-title {slug}-desc">
      <title id="{slug}-title">{html.escape(title)}</title>
      <desc id="{slug}-desc">{html.escape(desc)}</desc>
      <defs>{MARKERS}      </defs>
      <rect width="100%" height="100%" fill="{PAPER}"/>
{body}
    </svg>
    </div>
  </div>
</body>
</html>
'''


SVG_RE = re.compile(r"<svg\b.*?</svg>", re.S)


def export_svg(html_text: str) -> str:
    """Standalone SVG per diagram-design references/export.md."""
    svg = SVG_RE.search(html_text).group(0)
    style = f"<style>@import url('{FONTS_HREF.replace('&', '&amp;')}');</style>"
    if "<defs>" in svg:
        svg = svg.replace("<defs>", f"<defs>{style}", 1)
    else:
        svg = re.sub(r"(<desc[^>]*>.*?</desc>)", rf"\1<defs>{style}</defs>", svg, count=1, flags=re.S)
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + svg + "\n"


def write(stem, eyebrow, title, desc, vw, vh, body, project="", min_width=900):
    """Write <stem>.html and <stem>.svg. `stem` is a path without extension."""
    stem = Path(stem)
    slug = re.sub(r"[^a-z0-9]+", "-", stem.name.lower()).strip("-")
    text = page(slug, eyebrow, title, desc, vw, vh, body, project, min_width)
    stem.parent.mkdir(parents=True, exist_ok=True)
    stem.with_suffix(".html").write_text(text, encoding="utf-8")
    stem.with_suffix(".svg").write_text(export_svg(text), encoding="utf-8")
    return stem.with_suffix(".html")


if __name__ == "__main__":
    # Self-check: a tiny figure using every primitive family must round-trip.
    import tempfile
    b = [zone(24, 24, 400, 200, "ZONE"),
         hline(120, 100, 200, 100), label_above(160, 100, "Model"),
         elbow([(200, 160), (240, 160), (240, 200), (280, 200)], dashed=True, marker="arrow-open"),
         node(40, 72, 80, 56, "a", "x.rs:1", kind="focal", tag="FN", mono=True),
         node(200, 72, 80, 56, "b", kind="external", tag="EXT"),
         diamond(360, 120, "ok?"), ring(360, 200, "Done"), start(30, 100),
         cls(40, 240, 160, "C", ["+ f: u8"], ["+ g()"])[0],
         legend(440, 480, [(sw_box("focal"), "FOCAL"), (sw_line(), "CALL")])]
    with tempfile.TemporaryDirectory() as d:
        out = write(f"{d}/t", "Test", "t", "d", 480, 460, "\n".join(b), project="p")
        svg = out.with_suffix(".svg").read_text()
        assert svg.startswith("<?xml") and "&amp;family" in svg and "<title" in svg
        import xml.dom.minidom
        xml.dom.minidom.parseString(svg)
    print("draw.py ok")

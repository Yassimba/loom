#!/usr/bin/env python3
"""Assemble all built diagram HTML files into one readable page.

Usage: python3 assemble.py [WORKDIR]
WORKDIR holds atlas.json, diagrams/*/manifest.json, glossary.json, nav.css, nav.js.
Features: per-repo colour, collapsed level-3 zooms, collapsed build/deploy/test
figures per section, hover glossary terms, sidebar search, figure progress marker,
and per-diagram maximize and zoom controls.
"""
import html
import json
import re
import sys
from pathlib import Path

ROOT = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).parent
DIAG = ROOT / "diagrams"
OUT = ROOT / "atlas.html"
HERE = Path(__file__).parent
CFG = json.loads((ROOT / "atlas.json").read_text()) if (ROOT / "atlas.json").exists() else {}
TITLE = CFG.get("title", "System Atlas")
EYEBROW = CFG.get("eyebrow", "")
INTRO = CFG.get("intro", "")

SVG_RE = re.compile(r"(<svg\b.*?</svg>)", re.S)
OPS_RE = re.compile(r"\b(ci|cicd|ci/cd|release|docker|deploy|deployment|makefile|test structure|integration tests|versioning)\b", re.I)
LEVEL_NAME = {1: "overview", 2: "detail", 3: "deep dive"}
HUES = [18, 205, 145, 265, 340, 95, 40, 190, 300, 60, 230, 0]


def load_sections():
    secs = []
    for mf in DIAG.glob("*/manifest.json"):
        m = json.loads(mf.read_text())
        m["_dir"] = mf.parent
        secs.append(m)
    secs.sort(key=lambda s: s.get("order", 99))
    return secs



# ---------- SVG post-processing ----------
ID_RE = re.compile(r'\bid="([^"]+)"')
CM_RE = re.compile(r"color-mix\(in srgb, #([0-9a-fA-F]{6}) (\d+)%, transparent\)")
COLOR_ATTR_RE = re.compile(r'\b(fill|stroke|stop-color)="([^"]+)"')
# light palette -> dark palette (warm graphite). rgb triplets.
DARK = {"2d3142": (236, 231, 222), "4f5d75": (168, 161, 150), "f5f5f5": (38, 36, 33), "eb6c36": (240, 132, 79),
        "7a8399": (130, 124, 116), "bfc0c0": (74, 70, 64), "2e5aa8": (143, 179, 255), "ececec": (46, 44, 40), "ffffff": (38, 36, 33)}
SEEN_COLORS = set()


def _cm(m):
    h, pct = m.group(1), int(m.group(2))
    r, g, b = int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)
    return f"rgba({r},{g},{b},{pct/100:g})"


def svg_of(path: Path, fid: str = "") -> str:
    m = SVG_RE.search(path.read_text())
    if not m:
        raise SystemExit(f"no svg in {path}")
    svg = CM_RE.sub(_cm, m.group(1))
    if fid:
        ids = [i for i in ID_RE.findall(svg) if not i.startswith(fid)]
        for i in sorted(set(ids), key=len, reverse=True):
            svg = svg.replace(f'id="{i}"', f'id="{fid}-{i}"').replace(f"url(#{i})", f"url(#{fid}-{i})").replace(f'href="#{i}"', f'href="#{fid}-{i}"')
    for _, v in COLOR_ATTR_RE.findall(svg):
        SEEN_COLORS.add(v)
    return svg


def dark_color(v: str):
    v = v.strip()
    m = re.fullmatch(r"#([0-9a-fA-F]{6})", v)
    if m and m.group(1).lower() in DARK:
        r, g, b = DARK[m.group(1).lower()]; return f"rgb({r} {g} {b})"
    m = re.fullmatch(r"rgba\((\d+),\s*(\d+),\s*(\d+),\s*([0-9.]+)\)", v)
    if m:
        h = "%02x%02x%02x" % tuple(int(m.group(i)) for i in (1, 2, 3))
        if h in DARK:
            r, g, b = DARK[h]; return f"rgb({r} {g} {b} / {m.group(4)})"
    return None


def dark_svg_css() -> str:
    rules = []
    for v in sorted(SEEN_COLORS):
        d = dark_color(v)
        if d:
            rules.append(f'.svgwrap svg [fill="{v}"]{{fill:{d}}}.svgwrap svg [stroke="{v}"]{{stroke:{d}}}')
    body = "".join(rules)
    return ('@media (prefers-color-scheme: dark){:root:not([data-theme="light"]) {' + body + '}}\n'
            ':root[data-theme="dark"]{' + body + '}\n')


# ---------- glossary hover terms ----------
def load_glossary():
    gl = ROOT / "glossary.json"
    if not gl.exists():
        return {}, []
    g = json.loads(gl.read_text())
    aliases = []
    for t in g["terms"]:
        for a in re.split(r"\s+and\s+|,\s*|\s*/\s*", t["term"]):
            a = a.strip()
            if len(a) >= 3 and not a.lower().startswith("the "):
                aliases.append((a, t["meaning"]))
    aliases.sort(key=lambda x: -len(x[0]))
    return g, aliases


GLOSS, ALIASES = load_glossary()
ALIAS_RE = re.compile(r"\b(" + "|".join(re.escape(html.escape(a)) for a, _ in ALIASES) + r")\b", re.I) if ALIASES else None
ALIAS_MAP = {html.escape(a).lower(): m for a, m in ALIASES}


def mark_terms(escaped: str, seen: set) -> str:
    if not ALIAS_RE:
        return escaped
    def rep(m):
        key = m.group(1).lower()
        if key in seen:
            return m.group(1)
        seen.add(key)
        return f'<abbr class="term" tabindex="0" data-tip="{html.escape(ALIAS_MAP[key], quote=True)}">{m.group(1)}</abbr>'
    return ALIAS_RE.sub(rep, escaped)


def para(text: str, lead: bool = False, terms: bool = True) -> str:
    ps = [p.strip() for p in text.split("\n\n") if p.strip()]
    out, seen = [], set()
    for i, p in enumerate(ps):
        cls = ' class="caption-lead"' if lead and i == 0 else ""
        e = html.escape(p)
        if terms:
            e = mark_terms(e, seen)
        out.append(f"<p{cls}>{e}</p>")
    return "".join(out)


# ---------- figures ----------
def figure(sid, d, p, idx, total):
    fid = f"{sid}-{Path(d['file']).stem}"
    lvl = d.get("level", 2)
    return (
        f'<figure id="{fid}" class="l{lvl}" data-index="{idx}">'
        f'<div class="eyebrow">{html.escape(d.get("type", ""))} · {LEVEL_NAME.get(lvl, "deep dive")} · figure {idx} of {total}</div>'
        f'<h3>{html.escape(d["title"])}</h3>'
        f'<div class="svgwrap" role="region" tabindex="0" aria-label="{html.escape(d["title"], quote=True)} diagram. Scroll horizontally to see all content.">{svg_of(p, fid)}</div>'
        f'<figcaption>{para(d.get("caption", ""), lead=True)}</figcaption></figure>'
    ), fid, lvl


def main():
    secs = load_sections()
    total = sum(1 for s in secs for d in s["diagrams"] if (s["_dir"] / d["file"]).exists())
    toc, body, legend = [], [], []
    n = 0
    for si, s in enumerate(secs):
        sid = s["section"]
        hue = HUES[si % len(HUES)]
        legend.append(f'<a href="#{sid}" style="--h:{hue}"><i></i>{html.escape(re.split(r":| - | — ", s["title"])[0])}</a>')
        toc.append(f'<li style="--h:{hue}"><a href="#{sid}">{html.escape(s["title"])}</a><ol>')
        body.append(f'<section id="{sid}" style="--h:{hue}"><h2>{html.escape(s["title"])}</h2>{para(s.get("intro", ""))}')

        main_items, ops_items = [], []
        for d in s["diagrams"]:
            p = s["_dir"] / d["file"]
            if not p.exists():
                print("missing", p, file=sys.stderr)
                continue
            n += 1
            fhtml, fid, lvl = figure(sid, d, p, n, total)
            search = html.escape((d["title"] + " " + d.get("caption", "")).lower(), quote=True)
            is_ops = si > 0 and OPS_RE.search(d["title"]) is not None
            item = dict(html=fhtml, fid=fid, lvl=lvl, title=d["title"], search=search)
            (ops_items if is_ops else main_items).append(item)

        # group consecutive level-3 zooms under a <details>
        i = 0
        while i < len(main_items):
            it = main_items[i]
            toc.append(f'<li class="l{it["lvl"]}" data-search="{it["search"]}"><a href="#{it["fid"]}">{html.escape(it["title"])}</a></li>')
            body.append(it["html"])
            j = i + 1
            zooms = []
            while j < len(main_items) and main_items[j]["lvl"] >= 3:
                zooms.append(main_items[j]); j += 1
            if zooms:
                body.append(f'<details class="zooms"><summary>Show {len(zooms)} detail figure{"s" if len(zooms) > 1 else ""}</summary>')
                for z in zooms:
                    toc.append(f'<li class="l3" data-search="{z["search"]}"><a href="#{z["fid"]}">{html.escape(z["title"])}</a></li>')
                    body.append(z["html"])
                body.append("</details>")
            i = j
        if ops_items:
            body.append(f'<details class="ops"><summary>Build, deploy and tests ({len(ops_items)} figure{"s" if len(ops_items) > 1 else ""})</summary>')
            toc.append('<li class="l2 ops-head">Build, deploy and tests</li>')
            for it in ops_items:
                toc.append(f'<li class="l3" data-search="{it["search"]}"><a href="#{it["fid"]}">{html.escape(it["title"])}</a></li>')
                body.append(it["html"])
            body.append("</details>")
        toc.append("</ol></li>")
        body.append("</section>")

    glossary = ""
    if GLOSS:
        items = "".join(f'<div class="gi"><dt>{html.escape(t["term"])}</dt><dd>{html.escape(t["meaning"])}</dd></div>' for t in GLOSS["terms"])
        glossary = f'<section id="glossary"><h2>Words you will meet</h2>{para(GLOSS.get("intro", ""), terms=False)}<dl class="gloss">{items}</dl></section>'
        toc.append('<li><a href="#glossary">Words you will meet</a></li>')

    nav_css = ((ROOT / "nav.css") if (ROOT / "nav.css").exists() else (HERE / "nav.css")).read_text()
    nav_js = ((ROOT / "nav.js") if (ROOT / "nav.js").exists() else (HERE / "nav.js")).read_text()
    extra_css = ""  # page CSS lives in nav.css
    extra_js = ""  # page JS lives in nav.js
    page = f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(TITLE)}</title>
<link href="https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500;600&display=swap" rel="stylesheet">
<style>{nav_css}{extra_css}{dark_svg_css()}</style></head><body>
<nav aria-label="Table of contents"><div class="nav-head"><h1>{html.escape(TITLE)}</h1><div class="sub">{n} diagrams · {len(secs)} sections</div><button class="toc-mobile-toggle" type="button" aria-expanded="false" aria-controls="table-of-contents">Browse contents</button><div class="toc-status" aria-live="polite"><span>Now viewing</span><strong>Introduction</strong></div></div><ol class="toc-root" id="table-of-contents">{''.join(toc)}</ol></nav>
<main>
<div class="location-bar" role="navigation" aria-label="Current location"><span class="location-section">{html.escape(TITLE)}</span><span class="location-separator" aria-hidden="true">/</span><strong class="location-current">Introduction</strong></div>
<header class="hero"><div class="eyebrow">{html.escape(EYEBROW)} · generated {__import__('datetime').date.today()}</div>
<h1>{html.escape(TITLE)}</h1>
<p>{html.escape(INTRO)} Each repository keeps one colour on this page: in the sidebar, the section rule and the border of its overview figures. Overview figures are open; detail zooms and build-and-deploy figures sit behind a "Show" line. Dotted words show their meaning on hover. A glossary sits at the bottom.</p>
<div class="legend">{''.join(legend)}</div></header>
{''.join(body)}
{glossary}
</main><script>{nav_js}{extra_js}</script></body></html>"""
    OUT.write_text(page)
    print(f"wrote {OUT} ({OUT.stat().st_size/1e6:.1f} MB, {n} diagrams)")


if __name__ == "__main__":
    main()

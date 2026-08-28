#!/usr/bin/env python3
"""Build walkthrough.html from walkthrough.md with every SVG figure inlined.

    python3 build-html.py ai-docs/explanations/<slug>/walkthrough.md

Writes walkthrough.html next to the .md. Needs pandoc on PATH. Page chrome
matches the draw.py skin so figures and prose share one palette.
"""
from __future__ import annotations

import html
import re
import subprocess
import sys
from pathlib import Path

FONTS = ('<link href="https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1'
         '&family=Geist:wght@400;500;600&family=Geist+Mono:wght@400;500;600&display=swap" rel="stylesheet">')
CSS = """
:root{--paper:#f5f5f5;--ink:#2d3142;--muted:#4f5d75;--coral:#eb6c36;--line:#bfc0c0}
html{background:var(--paper);scrollbar-color:var(--muted) var(--paper)}
body{font-family:'Geist',sans-serif;background:var(--paper);color:var(--ink);line-height:1.55;padding:2rem 1.5rem 4rem;caret-color:var(--coral)}
h1,h2{font-family:'Instrument Serif',serif;font-weight:400;letter-spacing:-.01em}
h1{font-size:2.4rem;margin:0 0 1rem}h2{font-size:1.6rem;margin:2.5rem 0 .75rem;border-bottom:1px solid var(--line);padding-bottom:.25rem}
p,ul,ol{max-width:70ch;margin:.6rem 0}li{margin:.3rem 0}
code{font-family:'Geist Mono',monospace;font-size:.88em;color:var(--muted);background:#ebebeb;padding:.05em .3em;border-radius:3px}
a{color:var(--coral)}strong{color:var(--ink)}
figure{margin:1.25rem 0 1.5rem}figure svg{width:100%;height:auto;max-width:1400px;display:block}
figcaption{font-family:'Geist Mono',monospace;font-size:.75rem;color:var(--muted);margin-top:.4rem;letter-spacing:.04em}
::selection{background:var(--coral);color:#fff}:focus-visible{outline:2px solid var(--coral);outline-offset:2px}
"""
IMG_RE = re.compile(r'<img src="(?P<src>[^"]+\.svg)"[^>]*alt="(?P<alt>[^"]*)"[^>]*/?>')


def main() -> int:
    md = Path(sys.argv[1]).resolve()
    body = subprocess.run(["pandoc", str(md), "-f", "gfm", "-t", "html"], check=True,
                          capture_output=True, text=True).stdout
    count = 0

    def repl(m: re.Match) -> str:
        nonlocal count
        count += 1
        svg = (md.parent / m.group("src")).read_text(encoding="utf-8")
        if svg.startswith("<?xml"):
            svg = svg.split("\n", 1)[1]
        return f'<figure>{svg}<figcaption>{m.group("alt")}</figcaption></figure>'

    body = IMG_RE.sub(repl, body)
    title_m = re.search(r"^# (.+)$", md.read_text(encoding="utf-8"), re.M)
    title = html.escape(title_m.group(1)) if title_m else md.stem
    out = md.with_suffix(".html")
    out.write_text(
        '<!DOCTYPE html>\n<html lang="en">\n<head>\n<meta charset="UTF-8">\n'
        '<meta name="viewport" content="width=device-width, initial-scale=1.0">\n'
        f'<title>{title}</title>\n{FONTS}\n<style>{CSS}</style>\n</head>\n<body>\n{body}\n</body>\n</html>\n',
        encoding="utf-8")
    print(f"{out}: {count} figure(s) inlined")
    return 0 if count else 1


if __name__ == "__main__":
    sys.exit(main())
